//! Indoor (BLV) map spawner — the indoor equivalent of odm.rs.
//!
//! When the loading pipeline produces a `PreparedIndoorWorld`, this plugin
//! spawns the face-based geometry, door entities, and ambient lighting.
//! Also handles indoor face interaction (click/Enter) and door animation.

use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use lod::blv::DoorState;
use lod::odm::mm6_to_bevy;

use crate::GameState;
use crate::game::InGame;
use crate::game::entities::{actor, sprites};
use crate::game::event_dispatch::EventQueue;
use crate::game::events::MapEvents;
use crate::game::hud::HudView;
use crate::game::player::PlayerCamera;
use crate::game::raycast::{point_in_polygon, ray_plane_intersect};
use crate::states::loading::PreparedIndoorWorld;

// --- Components ---

/// Marker component on door face entities for animation.
#[derive(Component)]
pub struct DoorFace {
    pub door_index: usize,
    pub face_index: usize,
    /// Per triangle-vertex: whether it moves with the door.
    pub is_moving_vertex: Vec<bool>,
    /// Base vertex positions (Bevy coords) at door distance=0.
    /// Used directly for animation: moved_pos = base_pos + direction_bevy * distance.
    pub base_positions: Vec<[f32; 3]>,
    /// UV change per unit of door displacement for moving vertices.
    pub uv_rate: [f32; 2],
    /// Base UV values per triangle vertex (at distance=0).
    pub base_uvs: Vec<[f32; 2]>,
    /// Whether this face has the MOVES_BY_DOOR (FACE_TexMoveByDoor) attribute.
    pub moves_by_door: bool,
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
}

/// Resource holding all door runtime states for the current indoor map.
#[derive(Resource)]
pub struct BlvDoors {
    pub doors: Vec<DoorRuntime>,
}

/// Collision data for a single door face — a polygon that blocks movement.
pub struct DoorCollisionFace {
    pub door_index: usize,
    /// Base vertex positions (Bevy coords) at door distance=0.
    pub base_positions: Vec<Vec3>,
    /// Face normal in Bevy coords.
    pub normal: Vec3,
    /// Per-vertex: whether this vertex moves with the door.
    pub is_moving: Vec<bool>,
}

/// Dynamic collision resource for door faces.
/// Updated each frame by the door animation system.
#[derive(Resource, Default)]
pub struct DoorColliders {
    /// Source data for rebuilding collision each frame.
    pub face_data: Vec<DoorCollisionFace>,
    /// Current collision walls (rebuilt from face_data + door positions).
    pub walls: Vec<crate::game::collision::CollisionWall>,
}

impl DoorColliders {
    /// Push the player out of any door wall they would penetrate.
    /// Same algorithm as BuildingColliders::resolve_movement.
    pub fn resolve_movement(&self, from: Vec3, to: Vec3, radius: f32, eye_height: f32) -> Vec3 {
        let mut result = to;
        let feet_y = from.y - eye_height;

        for _ in 0..3 {
            let prev = result;

            for wall in &self.walls {
                if feet_y > wall.max_y || from.y < wall.min_y {
                    continue;
                }
                // Check if player is within the face's XZ footprint
                if result.x + radius < wall.min_x
                    || result.x - radius > wall.max_x
                    || result.z + radius < wall.min_z
                    || result.z - radius > wall.max_z
                {
                    continue;
                }

                let dist = wall.normal.dot(result) - wall.plane_dist;
                if dist < radius && dist > -radius {
                    let push = radius - dist;
                    result.x += wall.normal.x * push;
                    result.z += wall.normal.z * push;
                }
            }

            if (result.x - prev.x).abs() < 0.1 && (result.z - prev.z).abs() < 0.1 {
                break;
            }
        }

        result
    }
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

/// Info about a single touch-triggered indoor face.
pub struct TouchTriggerInfo {
    pub event_id: u16,
    pub center: Vec3,
    pub radius: f32,
}

/// Resource holding touch-triggered face data for the current indoor map.
#[derive(Resource)]
pub struct TouchTriggerFaces {
    pub faces: Vec<TouchTriggerInfo>,
    /// Track which events were already fired to avoid repeating every frame.
    pub fired: std::collections::HashSet<u16>,
}

// --- Plugin ---

pub struct BlvPlugin;

impl Plugin for BlvPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Game), spawn_indoor_world)
            .add_systems(
                Update,
                (
                    indoor_interact_system,
                    indoor_touch_trigger_system,
                    door_animation_system,
                )
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
    game_assets: Res<crate::assets::GameAssets>,
    cfg: Res<crate::config::GameConfig>,
) {
    let Some(prepared) = prepared else { return };

    // Load EVT events for this indoor map
    crate::game::events::load_map_events(&mut commands, &game_assets, &prepared.map_base, true);

    // Spawn all static face meshes (grouped by texture)
    let model_sampler = crate::assets::sampler_for_filtering(&cfg.models_filtering);
    for model in &prepared.models {
        for sub in &model.sub_meshes {
            let mut mat = sub.material.clone();
            if let Some(ref tex) = sub.texture {
                let mut img = tex.clone();
                img.sampler = model_sampler.clone();
                let tex_handle = images.add(img);
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
                is_moving_vertex: df.is_moving_vertex.clone(),
                base_positions: df.base_positions.clone(),
                uv_rate: df.uv_rate,
                base_uvs: df.base_uvs.clone(),
                moves_by_door: df.moves_by_door,
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
        })
        .collect();

    commands.insert_resource(BlvDoors { doors: door_runtimes });

    // Build DoorColliders from door face data for dynamic collision.
    // Each door face that is a wall (normal mostly horizontal) contributes
    // a collision polygon that moves with the door.
    let mut door_collision_faces = Vec::new();
    for df in &prepared.door_face_meshes {
        // Only wall-like faces block movement (normal.y close to 0)
        // Reconstruct polygon vertices from the triangle mesh base_positions.
        // Door faces are small enough that using all triangle vertices works.
        let n = df.base_positions.len();
        if n < 3 {
            continue;
        }

        // Compute face normal from first triangle
        let p0 = Vec3::from(df.base_positions[0]);
        let p1 = Vec3::from(df.base_positions[1]);
        let p2 = Vec3::from(df.base_positions[2]);
        let normal = (p1 - p0).cross(p2 - p0).normalize_or_zero();
        if normal.y.abs() > 0.7 {
            continue;
        } // Skip floors/ceilings

        door_collision_faces.push(DoorCollisionFace {
            door_index: df.door_index,
            base_positions: df.base_positions.iter().map(|p| Vec3::from(*p)).collect(),
            normal,
            is_moving: df.is_moving_vertex.clone(),
        });
    }
    commands.insert_resource(DoorColliders {
        face_data: door_collision_faces,
        walls: Vec::new(),
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

    // Build TouchTriggerFaces resource
    let touch_faces: Vec<TouchTriggerInfo> = prepared
        .touch_trigger_faces
        .iter()
        .map(|tf| TouchTriggerInfo {
            event_id: tf.event_id,
            center: tf.center,
            radius: tf.radius,
        })
        .collect();
    if !touch_faces.is_empty() {
        info!("Indoor map has {} touch-trigger faces", touch_faces.len());
    }
    commands.insert_resource(TouchTriggerFaces {
        faces: touch_faces,
        fired: std::collections::HashSet::new(),
    });

    // Indoor ambient lighting — dim for dungeon atmosphere.
    // TODO: read per-sector light levels from BLV data for proper local lighting.
    commands.spawn((
        AmbientLight {
            color: Color::srgb(0.7, 0.65, 0.55),
            brightness: 50.0,
            ..default()
        },
        InGame,
    ));

    // Spawn NPC actors from DLV data using the pre-resolved Actors abstraction.
    let mut sprite_cache = sprites::SpriteCache::default();

    if let Ok(actors) = lod::game::actors::Actors::from_raw_actors(
        game_assets.lod_manager(),
        &prepared.actors,
        None,
        game_assets.game_data(),
    ) {
        for actor in actors.get_actors() {
            let variant = actor.variant;

            let (states, state_masks, sw, sh) = sprites::load_entity_sprites(
                &actor.standing_sprite,
                &actor.walking_sprite,
                game_assets.lod_manager(),
                &mut images,
                &mut materials,
                &mut Some(&mut sprite_cache),
                variant,
                actor.palette_id,
            );
            if states.is_empty() || states[0].is_empty() {
                error!(
                    "Indoor NPC '{}' monlist_id={} sprite '{}'/'{}'  failed to load",
                    actor.name, actor.monlist_id, actor.standing_sprite, actor.walking_sprite
                );
                continue;
            }
            let initial_mat = states[0][0][0].clone();
            let quad = meshes.add(Rectangle::new(sw, sh));
            // Indoor actors use MM6 coordinates directly (no heightmap probing)
            let [bx, by, bz] = mm6_to_bevy(actor.position[0], actor.position[1], actor.position[2]);
            let pos = Vec3::new(bx, by + sh / 2.0, bz);

            commands.spawn((
                Name::new(format!("npc:{}", actor.name)),
                Mesh3d(quad),
                MeshMaterial3d(initial_mat),
                Transform::from_translation(pos),
                crate::game::entities::WorldEntity,
                crate::game::entities::EntityKind::Npc,
                crate::game::entities::AnimationState::Idle,
                sprites::SpriteSheet::new(states, vec![(sw, sh)], state_masks),
                actor::Actor {
                    name: actor.name.clone(),
                    hp: actor.hp,
                    max_hp: actor.hp,
                    move_speed: actor.move_speed as f32,
                    initial_position: pos,
                    guarding_position: pos,
                    tether_distance: actor.tether_distance as f32,
                    wander_timer: (pos.x * 0.011 + pos.z * 0.017).abs().fract() * 4.0,
                    wander_target: pos,
                    facing_yaw: 0.0,
                    hostile: false,
                },
                crate::game::interaction::NpcInteractable {
                    name: actor.name.clone(),
                    position: pos,
                    npc_id: actor.npc_id(),
                },
                crate::game::entities::Billboard,
                InGame,
            ));
            info!("Spawned indoor NPC '{}' at {:?}", actor.name, pos);
        }
    }
}

// --- Indoor face interaction ---

const INDOOR_INTERACT_RANGE: f32 = 5120.0;

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
    let gamepad = gamepads
        .iter()
        .any(|gp| gp.just_pressed(bevy::input::gamepad::GamepadButton::East));
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

    let Ok((cam_global, _)) = camera_query.single() else {
        return;
    };
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
            if point_in_polygon(hit_point, &face.vertices, face.normal)
                && (nearest_hit.is_none() || t < nearest_hit.unwrap().0)
            {
                nearest_hit = Some((t, face.event_id));
            }
        }
    }

    if let Some((dist, event_id)) = nearest_hit {
        info!("Indoor interact: hit face event_id={} at dist={:.0}", event_id, dist);
        let Some(me) = map_events else {
            warn!("Indoor interact: no MapEvents resource");
            return;
        };
        let Some(evt) = me.evt.as_ref() else {
            warn!("Indoor interact: no EVT file loaded");
            return;
        };
        if let Some(steps) = evt.events.get(&event_id) {
            info!(
                "Indoor interact: dispatching {} steps for event_id={}",
                steps.len(),
                event_id
            );
        } else {
            info!("Indoor interact: no actions found for event_id={}", event_id);
        }
        event_queue.push_all(event_id, evt);
    }
}

// --- Touch trigger (EVENT_BY_TOUCH proximity) ---

/// Check player proximity to touch-triggered faces and dispatch events.
fn indoor_touch_trigger_system(
    player_query: Query<&Transform, With<crate::game::player::Player>>,
    mut touch_triggers: Option<ResMut<TouchTriggerFaces>>,
    map_events: Option<Res<MapEvents>>,
    mut event_queue: ResMut<EventQueue>,
) {
    let Some(ref mut triggers) = touch_triggers else { return };
    if triggers.faces.is_empty() {
        return;
    }
    let Ok(player_tf) = player_query.single() else { return };
    let player_pos = player_tf.translation;

    // Collect events to fire (avoids borrow conflict with fired set)
    let to_fire: Vec<u16> = triggers
        .faces
        .iter()
        .filter(|f| !triggers.fired.contains(&f.event_id))
        .filter(|f| player_pos.distance(f.center) <= f.radius)
        .map(|f| f.event_id)
        .collect();

    if let Some(me) = map_events.as_ref()
        && let Some(evt) = me.evt.as_ref()
    {
        for eid in &to_fire {
            info!("Touch trigger: event_id={}", eid);
            event_queue.push_all(*eid, evt);
        }
    }
    for eid in to_fire {
        triggers.fired.insert(eid);
    }

    // Reset fired events when player moves away (allows re-triggering)
    let still_near: std::collections::HashSet<u16> = triggers
        .faces
        .iter()
        .filter(|f| triggers.fired.contains(&f.event_id) && player_pos.distance(f.center) <= f.radius * 1.5)
        .map(|f| f.event_id)
        .collect();
    triggers.fired = still_near;
}

// --- Door trigger ---

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

// --- Door animation ---

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
            // Opening = moving toward Open (displacement decreasing: move_length → 0)
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
            // Closing = moving toward Closed (displacement increasing: 0 → move_length)
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
fn door_animation_system(
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

        // Convert MM6 direction to Bevy coords: mm6(x,y,z) → bevy(x,z,-y)
        let dir = [door.direction[0], door.direction[2], -door.direction[1]];

        let Some(mesh) = meshes.get_mut(&mesh_handle.0) else {
            continue;
        };

        // Update vertex positions using stored base positions (Bevy coords).
        // base_positions are the open/retracted positions; closed = base + dir * distance.
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

        // Update UVs for faces with the MOVES_BY_DOOR (FACE_TexMoveByDoor) flag.
        // These are typically reveal/frame faces where some vertices move and some are fixed.
        // For pure panel faces (all moving, no flag), the texture moves with geometry naturally.
        //
        // OpenEnroth formula: textureDelta = -dot(direction, axis) * distance + baseDelta
        // Our per-vertex equivalent: scroll moving vertex UVs opposite to geometric movement
        // to keep the texture aligned with the face plane as it deforms.
        if face.moves_by_door
            && face.uv_rate != [0.0, 0.0]
            && let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0)
        {
            for (vi, uv) in uvs.iter_mut().enumerate() {
                if face.is_moving_vertex.get(vi).copied().unwrap_or(false)
                    && let Some(base) = face.base_uvs.get(vi)
                {
                    uv[0] = base[0] - face.uv_rate[0] * distance;
                    uv[1] = base[1] - face.uv_rate[1] * distance;
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

            // Only block when the door is not fully open
            let distance = door_slide_distance(door);
            if distance < 1.0 {
                continue;
            } // Fully open — no collision

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
    }
}
