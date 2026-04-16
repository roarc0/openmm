//! Door animation and triggering.

use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;

use openmm_data::blv::DoorState;

use super::types::{BlvDoors, DoorColliders, DoorFace, DoorRuntime};

/// Trigger a door state change. Called from event_dispatch when ChangeDoorState fires.
///
/// MM6 SetDoorState actions (from MMExtension):
///   0 = go to state (0) = Open position (initial)
///   1 = go to state (1) = Closed position (alternate)
///   2 = toggle if door isn't moving
///   3 = toggle always
pub fn trigger_door(doors: &mut BlvDoors, door_id: u32, action: u8) {
    // For toggle (action=2 or 3), resolve to open/close first
    let resolved_action = if action == 2 || action == 3 {
        let Some(door) = doors.doors.iter().find(|d| d.door_id == door_id) else {
            warn!("trigger_door: no door with id={}", door_id);
            return;
        };
        // action=2: only toggle if fully open/closed (not moving)
        if action == 2 && matches!(door.state, DoorState::Opening | DoorState::Closing) {
            return;
        }
        match door.state {
            DoorState::Closed | DoorState::Closing => 0u8, // -> Open (state 0)
            DoorState::Open | DoorState::Opening => 1u8,   // -> Close (state 1)
        }
    } else {
        action
    };

    let Some(door) = doors.doors.iter_mut().find(|d| d.door_id == door_id) else {
        warn!("trigger_door: no door with id={}", door_id);
        return;
    };

    match resolved_action {
        0 => {
            // Go to state (0) = Open position
            match door.state {
                DoorState::Closed | DoorState::Closing => {
                    if door.state == DoorState::Closing && door.move_length > 0 && door.open_speed > 0 {
                        let total_close_time = (door.move_length as f32 / door.close_speed as f32) * 1000.0;
                        let progress = (door.time_since_triggered_ms / total_close_time).clamp(0.0, 1.0);
                        let total_open_time = (door.move_length as f32 / door.open_speed as f32) * 1000.0;
                        door.time_since_triggered_ms = total_open_time * (1.0 - progress);
                    } else {
                        door.time_since_triggered_ms = 0.0;
                    }
                    door.state = DoorState::Opening;
                }
                _ => {}
            }
        }
        1 => {
            // Go to state (1) = Closed position
            match door.state {
                DoorState::Open | DoorState::Opening => {
                    if door.state == DoorState::Opening && door.move_length > 0 && door.close_speed > 0 {
                        let total_open_time = (door.move_length as f32 / door.open_speed as f32) * 1000.0;
                        let progress = (door.time_since_triggered_ms / total_open_time).clamp(0.0, 1.0);
                        let total_close_time = (door.move_length as f32 / door.close_speed as f32) * 1000.0;
                        door.time_since_triggered_ms = total_close_time * (1.0 - progress);
                    } else {
                        door.time_since_triggered_ms = 0.0;
                    }
                    door.state = DoorState::Closing;
                }
                _ => {}
            }
        }
        _ => {
            warn!("trigger_door: unknown action={}", resolved_action);
        }
    }
}

/// Calculate the slide distance for a door based on its current state and time.
///
/// Returns the displacement from the "open" (base) position:
///   0.0 = fully open (vertices at BLV positions)
///   move_length = fully closed (vertices displaced by direction * move_length)
fn door_slide_distance(door: &DoorRuntime) -> f32 {
    match door.state {
        DoorState::Open => 0.0,
        DoorState::Closed => door.move_length as f32,
        DoorState::Opening => {
            // Opening = moving toward Open (displacement decreasing: move_length -> 0)
            let total_time = if door.open_speed > 0 {
                (door.move_length as f32 / door.open_speed as f32) * 1000.0
            } else {
                0.0
            };
            let progress = if total_time > 0.0 {
                (door.time_since_triggered_ms / total_time).clamp(0.0, 1.0)
            } else {
                1.0
            };
            (1.0 - progress) * door.move_length as f32
        }
        DoorState::Closing => {
            // Closing = moving toward Closed (displacement increasing: 0 -> move_length)
            let total_time = if door.close_speed > 0 {
                (door.move_length as f32 / door.close_speed as f32) * 1000.0
            } else {
                0.0
            };
            let progress = if total_time > 0.0 {
                (door.time_since_triggered_ms / total_time).clamp(0.0, 1.0)
            } else {
                1.0
            };
            progress * door.move_length as f32
        }
    }
}

/// Advance door animations, update mesh vertex positions, and rebuild door collision.
pub(crate) fn door_animation_system(
    time: Res<Time>,
    mut blv_doors: Option<ResMut<BlvDoors>>,
    door_faces: Query<(&DoorFace, &Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut door_colliders: Option<ResMut<DoorColliders>>,
) {
    let Some(ref mut doors) = blv_doors else { return };
    let dt_ms = time.delta_secs() * 1000.0;

    // First pass: advance door timers
    for door in &mut doors.doors {
        match door.state {
            DoorState::Opening => {
                door.time_since_triggered_ms += dt_ms;
                let total_time = if door.open_speed > 0 {
                    (door.move_length as f32 / door.open_speed as f32) * 1000.0
                } else {
                    0.0
                };
                if door.time_since_triggered_ms >= total_time {
                    door.time_since_triggered_ms = total_time;
                    door.state = DoorState::Open;
                }
            }
            DoorState::Closing => {
                door.time_since_triggered_ms += dt_ms;
                let total_time = if door.close_speed > 0 {
                    (door.move_length as f32 / door.close_speed as f32) * 1000.0
                } else {
                    0.0
                };
                if door.time_since_triggered_ms >= total_time {
                    door.time_since_triggered_ms = total_time;
                    door.state = DoorState::Closed;
                }
            }
            _ => {} // Open/Closed: no timer change needed
        }
    }

    // Second pass: update mesh positions for all door faces
    for (face, mesh_handle) in door_faces.iter() {
        let Some(door) = doors.doors.get(face.door_index) else {
            continue;
        };

        let distance = door_slide_distance(door);

        // Convert MM6 direction to Bevy coords: mm6(x,y,z) -> bevy(x,z,-y)
        let dir = [door.direction[0], door.direction[2], -door.direction[1]];

        let Some(mesh) = meshes.get_mut(&mesh_handle.0) else {
            continue;
        };

        // Update vertex positions using stored base positions (Bevy coords).
        // base_positions are the BLV (deployed/blocking) positions; retracted = base + dir * distance.
        if let Some(VertexAttributeValues::Float32x3(positions)) = mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION) {
            for (vi, pos) in positions.iter_mut().enumerate() {
                if !face.is_moving_vertex.get(vi).copied().unwrap_or(false) {
                    continue;
                }
                let Some(base) = face.base_positions.get(vi) else {
                    continue;
                };
                pos[0] = base[0] + dir[0] * distance;
                pos[1] = base[1] + dir[1] * distance;
                pos[2] = base[2] + dir[2] * distance;
            }
        }

        // Update UVs for faces where texture must track geometry deformation.
        //
        // Three cases based on which vertices move:
        // 1. Pure panel (all vertices moving): no correction needed — the texture is carried
        //    rigidly with the mesh. UVs stay correct without any update.
        // 2. MOVES_BY_DOOR flag (reveal/frame face): fixed vertices anchor the texture while
        //    moving vertices pull it. Scroll OPPOSITE to motion: base - rate*distance.
        // 3. Hybrid face (some vertices move, no flag): deforming geometry. Moving vertices
        //    need UV correction to stay aligned: base + rate*distance.
        let all_moving = face.is_moving_vertex.iter().all(|&m| m);
        if !all_moving
            && face.uv_rate != [0.0, 0.0]
            && let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0)
        {
            for (vi, uv) in uvs.iter_mut().enumerate() {
                if face.is_moving_vertex.get(vi).copied().unwrap_or(false)
                    && let Some(base) = face.base_uvs.get(vi)
                {
                    if face.moves_by_door {
                        // Frame face: scroll texture counter to motion so it doesn't stretch.
                        uv[0] = base[0] - face.uv_rate[0] * distance;
                        uv[1] = base[1] - face.uv_rate[1] * distance;
                    } else {
                        // Hybrid face: advance UV with the moving vertex to cancel deformation.
                        uv[0] = base[0] + face.uv_rate[0] * distance;
                        uv[1] = base[1] + face.uv_rate[1] * distance;
                    }
                }
            }
        }
    }

    // Third pass: rebuild door collision walls from face data
    if let Some(ref mut colliders) = door_colliders {
        // Build new walls into a temporary vec to avoid borrow conflict
        let mut new_walls = Vec::new();
        for cf in &colliders.face_data {
            let Some(door) = doors.doors.get(cf.door_index) else {
                continue;
            };

            // Only skip collision when the door is fully retracted (passable).
            let distance = door_slide_distance(door);
            if door.move_length == 0 || distance >= door.move_length as f32 - 1.0 {
                continue; // Door fully retracted — no collision
            }

            let dir_bevy = Vec3::new(door.direction[0], door.direction[2], -door.direction[1]);

            // Compute current vertex positions (applying door displacement to moving verts)
            let current_verts: Vec<Vec3> = cf
                .base_positions
                .iter()
                .enumerate()
                .map(|(vi, base)| {
                    if cf.is_moving.get(vi).copied().unwrap_or(false) {
                        *base + dir_bevy * distance
                    } else {
                        *base
                    }
                })
                .collect();

            if current_verts.len() < 3 {
                continue;
            }

            // Deduplicate vertices (triangulated meshes have duplicates)
            let mut unique_verts: Vec<Vec3> = Vec::new();
            for v in &current_verts {
                let is_dup = unique_verts.iter().any(|u| (*u - *v).length_squared() < 1.0);
                if !is_dup {
                    unique_verts.push(*v);
                }
            }
            if unique_verts.len() < 3 {
                continue;
            }

            let plane_dist = cf.normal.dot(unique_verts[0]);
            new_walls.push(crate::game::collision::CollisionWall::new(
                cf.normal,
                plane_dist,
                &unique_verts,
            ));
        }
        colliders.walls = new_walls;

        // Rebuild dynamic ceilings from horizontal door faces.
        // A horizontal panel at height Y blocks the player when Y is between their feet and eyes.
        let mut new_ceilings = Vec::new();
        for cf in &colliders.horizontal_face_data {
            let Some(door) = doors.doors.get(cf.door_index) else {
                continue;
            };
            let distance = door_slide_distance(door);
            if door.move_length == 0 || distance >= door.move_length as f32 - 1.0 {
                continue; // Door fully retracted — no collision
            }

            let dir_bevy = Vec3::new(door.direction[0], door.direction[2], -door.direction[1]);

            // Mesh vertices come in triangle triples; compute current positions for each
            for (chunk_idx, chunk) in cf.base_positions.chunks(3).enumerate() {
                if chunk.len() < 3 {
                    continue;
                }
                let base_vi = chunk_idx * 3;
                let v: Vec<Vec3> = chunk
                    .iter()
                    .enumerate()
                    .map(|(i, base)| {
                        let vi = base_vi + i;
                        if cf.is_moving.get(vi).copied().unwrap_or(false) {
                            *base + dir_bevy * distance
                        } else {
                            *base
                        }
                    })
                    .collect();
                new_ceilings.push(crate::game::collision::CollisionTriangle::new(
                    v[0], v[1], v[2], cf.normal,
                ));
            }
        }
        colliders.dynamic_ceilings = new_ceilings;
    }
}
