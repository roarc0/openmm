//! Indoor (BLV) map spawner — the indoor equivalent of odm.rs.
//!
//! When the loading pipeline produces a `PreparedIndoorWorld`, this plugin
//! spawns the face-based geometry, door entities, and ambient lighting.
//! Also handles indoor face interaction (click/Enter) and door animation.

use bevy::prelude::*;
use bevy::mesh::VertexAttributeValues;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use lod::blv::DoorState;
use lod::odm::mm6_to_bevy;

use crate::game::event_dispatch::EventQueue;
use crate::game::events::MapEvents;
use crate::game::hud::HudView;
use crate::game::player::PlayerCamera;
use crate::game::InGame;
use crate::states::loading::PreparedIndoorWorld;
use crate::GameState;

// --- Components ---

/// Marker component on door face entities for animation.
#[derive(Component)]
pub struct DoorFace {
    pub door_index: usize,
    pub face_index: usize,
    /// For each triangle vertex, the index into door.vertex_ids/offsets.
    pub vertex_door_indices: Vec<usize>,
}

// --- Resources ---

/// Runtime state for a single door.
pub struct DoorRuntime {
    pub door_id: u32,
    /// Direction vector in MM6 coordinates.
    pub direction: [f32; 3],
    pub move_length: i32,
    pub open_speed: i32,
    pub close_speed: i32,
    pub state: DoorState,
    /// Milliseconds since last state change.
    pub time_since_triggered_ms: f32,
    pub vertex_ids: Vec<u16>,
    pub x_offsets: Vec<i16>,
    pub y_offsets: Vec<i16>,
    pub z_offsets: Vec<i16>,
}

/// Resource holding all door runtime states for the current indoor map.
#[derive(Resource)]
pub struct BlvDoors {
    pub doors: Vec<DoorRuntime>,
}

/// Info about a single clickable indoor face.
pub struct ClickableFaceInfo {
    pub face_index: usize,
    pub event_id: u16,
    pub normal: Vec3,
    pub plane_dist: f32,
    pub vertices: Vec<Vec3>,
}

/// Resource holding all clickable face data for the current indoor map.
#[derive(Resource)]
pub struct ClickableFaces {
    pub faces: Vec<ClickableFaceInfo>,
}

// --- Plugin ---

pub struct BlvPlugin;

impl Plugin for BlvPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Game), spawn_indoor_world)
            .add_systems(
                Update,
                (indoor_interact_system, door_animation_system)
                    .run_if(in_state(GameState::Game))
                    .run_if(resource_equals(HudView::World)),
            );
    }
}

// --- Spawn ---

fn spawn_indoor_world(
    prepared: Option<Res<PreparedIndoorWorld>>,
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(prepared) = prepared else { return };

    // Spawn all static face meshes (grouped by texture)
    for model in &prepared.models {
        for sub in &model.sub_meshes {
            let mut mat = sub.material.clone();
            if let Some(ref tex) = sub.texture {
                let tex_handle = images.add(tex.clone());
                mat.base_color_texture = Some(tex_handle);
            }
            commands.spawn((
                Mesh3d(meshes.add(sub.mesh.clone())),
                MeshMaterial3d(materials.add(mat)),
                InGame,
            ));
        }
    }

    // Spawn door face entities individually (for animation)
    for df in &prepared.door_face_meshes {
        let mut mat = df.material.clone();
        if let Some(ref tex) = df.texture {
            let tex_handle = images.add(tex.clone());
            mat.base_color_texture = Some(tex_handle);
        }
        commands.spawn((
            Mesh3d(meshes.add(df.mesh.clone())),
            MeshMaterial3d(materials.add(mat)),
            DoorFace {
                door_index: df.door_index,
                face_index: df.face_index,
                vertex_door_indices: df.vertex_door_indices.clone(),
            },
            InGame,
        ));
    }

    // Build BlvDoors resource from prepared door data.
    // Preserve indices so DoorFace.door_index matches directly.
    let door_runtimes: Vec<DoorRuntime> = prepared
        .doors
        .iter()
        .map(|door| DoorRuntime {
            door_id: door.door_id,
            direction: door.direction,
            move_length: door.move_length,
            open_speed: door.open_speed,
            close_speed: door.close_speed,
            state: door.state,
            time_since_triggered_ms: match door.state {
                DoorState::Open | DoorState::Closed => {
                    if door.move_length > 0 && door.open_speed > 0 {
                        (door.move_length as f32 / door.open_speed as f32) * 1000.0
                    } else {
                        0.0
                    }
                }
                _ => 0.0,
            },
            vertex_ids: door.vertex_ids.clone(),
            x_offsets: door.x_offsets.clone(),
            y_offsets: door.y_offsets.clone(),
            z_offsets: door.z_offsets.clone(),
        })
        .collect();

    commands.insert_resource(BlvDoors {
        doors: door_runtimes,
    });

    // Build ClickableFaces resource
    let faces: Vec<ClickableFaceInfo> = prepared
        .clickable_faces
        .iter()
        .map(|cf| ClickableFaceInfo {
            face_index: cf.face_index,
            event_id: cf.event_id,
            normal: cf.normal,
            plane_dist: cf.plane_dist,
            vertices: cf.vertices.clone(),
        })
        .collect();
    commands.insert_resource(ClickableFaces { faces });

    // Indoor ambient lighting
    commands.spawn((
        AmbientLight {
            color: Color::srgb(0.9, 0.85, 0.75),
            brightness: 800.0,
            ..default()
        },
        InGame,
    ));
    commands.spawn((
        PointLight {
            intensity: 2_000_000.0,
            range: 50_000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 2000.0, 0.0),
        InGame,
    ));
}

// --- Indoor face interaction ---

/// Ray-plane intersection. Returns distance along ray if hit (positive = in front).
fn ray_plane_intersect(origin: Vec3, dir: Vec3, normal: Vec3, plane_dist: f32) -> Option<f32> {
    let denom = normal.dot(dir);
    if denom.abs() < 1e-6 {
        return None; // Ray parallel to plane
    }
    let t = (plane_dist - normal.dot(origin)) / denom;
    if t > 0.0 { Some(t) } else { None }
}

/// Test if a point lies inside a convex/concave polygon using winding number.
/// All points are assumed coplanar. Projects to the best 2D plane based on normal.
fn point_in_polygon(point: Vec3, vertices: &[Vec3], normal: Vec3) -> bool {
    if vertices.len() < 3 {
        return false;
    }
    // Choose projection axes: drop the axis with the largest normal component
    let abs_n = normal.abs();
    let (ax1, ax2) = if abs_n.x >= abs_n.y && abs_n.x >= abs_n.z {
        // Drop X, project to YZ
        (1usize, 2usize)
    } else if abs_n.y >= abs_n.z {
        // Drop Y, project to XZ
        (0, 2)
    } else {
        // Drop Z, project to XY
        (0, 1)
    };

    let get = |v: Vec3, axis: usize| -> f32 {
        match axis {
            0 => v.x,
            1 => v.y,
            _ => v.z,
        }
    };

    let px = get(point, ax1);
    let py = get(point, ax2);

    // Winding number test
    let mut winding = 0i32;
    let n = vertices.len();
    for i in 0..n {
        let v1 = vertices[i];
        let v2 = vertices[(i + 1) % n];
        let y1 = get(v1, ax2);
        let y2 = get(v2, ax2);

        if y1 <= py {
            if y2 > py {
                // Upward crossing
                let x1 = get(v1, ax1);
                let x2 = get(v2, ax1);
                let cross = (x2 - x1) * (py - y1) - (px - x1) * (y2 - y1);
                if cross > 0.0 {
                    winding += 1;
                }
            }
        } else if y2 <= py {
            // Downward crossing
            let x1 = get(v1, ax1);
            let x2 = get(v2, ax1);
            let cross = (x2 - x1) * (py - y1) - (px - x1) * (y2 - y1);
            if cross < 0.0 {
                winding -= 1;
            }
        }
    }

    winding != 0
}

const INDOOR_INTERACT_RANGE: f32 = 2000.0;

/// Detect indoor face interaction (Enter/click) and dispatch EVT events.
fn indoor_interact_system(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    camera_query: Query<(&GlobalTransform, &Camera), With<PlayerCamera>>,
    clickable: Option<Res<ClickableFaces>>,
    map_events: Option<Res<MapEvents>>,
    mut event_queue: ResMut<EventQueue>,
    cursor_query: Query<&CursorOptions, With<PrimaryWindow>>,
) {
    let Some(clickable) = clickable else { return };
    if clickable.faces.is_empty() {
        return;
    }

    // Check input
    let key = keys.just_pressed(KeyCode::KeyE) || keys.just_pressed(KeyCode::Enter);
    let click = mouse.just_pressed(MouseButton::Left);
    let gamepad = gamepads.iter().any(|gp| gp.just_pressed(bevy::input::gamepad::GamepadButton::East));
    if !key && !click && !gamepad {
        return;
    }

    // Don't process click if cursor isn't grabbed
    if click {
        let cursor_grabbed = cursor_query
            .single()
            .map(|c| !matches!(c.grab_mode, CursorGrabMode::None))
            .unwrap_or(true);
        if !cursor_grabbed {
            return;
        }
    }

    let Ok((cam_global, _)) = camera_query.single() else { return };
    let ray_origin = cam_global.translation();
    let ray_dir = cam_global.forward().as_vec3();

    // Find nearest clickable face hit
    let mut nearest_hit: Option<(f32, u16)> = None;
    for face in &clickable.faces {
        if let Some(t) = ray_plane_intersect(ray_origin, ray_dir, face.normal, face.plane_dist) {
            if t > INDOOR_INTERACT_RANGE {
                continue;
            }
            let hit_point = ray_origin + ray_dir * t;
            if point_in_polygon(hit_point, &face.vertices, face.normal) {
                if nearest_hit.is_none() || t < nearest_hit.unwrap().0 {
                    nearest_hit = Some((t, face.event_id));
                }
            }
        }
    }

    if let Some((_, event_id)) = nearest_hit {
        let Some(me) = map_events else { return };
        let Some(evt) = me.evt.as_ref() else { return };
        event_queue.push_all(event_id, evt);
    }
}

// --- Door trigger ---

/// Trigger a door state change. Called from event_dispatch when ChangeDoorState fires.
pub fn trigger_door(doors: &mut BlvDoors, door_id: u32, action: u8) {
    // For toggle (action=2), resolve to open/close first to avoid borrow issues
    let resolved_action = if action == 2 {
        let Some(door) = doors.doors.iter().find(|d| d.door_id == door_id) else {
            warn!("trigger_door: no door with id={}", door_id);
            return;
        };
        match door.state {
            DoorState::Closed | DoorState::Closing => 1u8, // Open
            DoorState::Open | DoorState::Opening => 0u8,   // Close
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
            // Close
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
        1 => {
            // Open
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
        _ => {
            warn!("trigger_door: unknown action={}", resolved_action);
        }
    }
}

// --- Door animation ---

/// Calculate the slide distance for a door based on its current state and time.
fn door_slide_distance(door: &DoorRuntime) -> f32 {
    let total_time = match door.state {
        DoorState::Opening | DoorState::Open => {
            if door.open_speed > 0 {
                (door.move_length as f32 / door.open_speed as f32) * 1000.0
            } else {
                0.0
            }
        }
        DoorState::Closing | DoorState::Closed => {
            if door.close_speed > 0 {
                (door.move_length as f32 / door.close_speed as f32) * 1000.0
            } else {
                0.0
            }
        }
    };

    let progress = if total_time > 0.0 {
        (door.time_since_triggered_ms / total_time).clamp(0.0, 1.0)
    } else {
        1.0
    };

    match door.state {
        DoorState::Opening | DoorState::Open => progress * door.move_length as f32,
        DoorState::Closing | DoorState::Closed => (1.0 - progress) * door.move_length as f32,
    }
}

/// Advance door animations and update mesh vertex positions.
fn door_animation_system(
    time: Res<Time>,
    mut blv_doors: Option<ResMut<BlvDoors>>,
    door_faces: Query<(&DoorFace, &Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
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

    // Second pass: update mesh positions for animating doors
    for (face, mesh_handle) in door_faces.iter() {
        let Some(door) = doors.doors.get(face.door_index) else { continue };

        // Skip doors with no vertex data or not animating
        if door.vertex_ids.is_empty() {
            continue;
        }
        match door.state {
            DoorState::Opening | DoorState::Closing => {}
            _ => continue,
        }

        let distance = door_slide_distance(door);

        let Some(mesh) = meshes.get_mut(&mesh_handle.0) else { continue };
        if let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
        {
            for (vi, pos) in positions.iter_mut().enumerate() {
                let door_vi = face.vertex_door_indices.get(vi).copied().unwrap_or(0);

                // Base position from door offsets (MM6 coords)
                let base_x = door.x_offsets.get(door_vi).copied().unwrap_or(0) as f32;
                let base_y = door.y_offsets.get(door_vi).copied().unwrap_or(0) as f32;
                let base_z = door.z_offsets.get(door_vi).copied().unwrap_or(0) as f32;

                // Apply direction * distance (in MM6 coords)
                let moved_x = base_x + door.direction[0] * distance;
                let moved_y = base_y + door.direction[1] * distance;
                let moved_z = base_z + door.direction[2] * distance;

                *pos = mm6_to_bevy(moved_x as i32, moved_y as i32, moved_z as i32);
            }
        }
    }
}
