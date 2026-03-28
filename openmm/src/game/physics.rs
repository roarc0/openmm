use bevy::prelude::*;

use crate::GameState;
use crate::game::collision::{
    BuildingColliders, CollisionTriangle, CollisionWall, TerrainHeightMap, WaterMap, WaterWalking,
    ground_height_at, sample_terrain_height,
};
use crate::game::player::{FlyMode, Player, PlayerPhysics, PlayerSettings};
use crate::states::loading::PreparedWorld;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Game), setup_collision_data)
            .add_systems(
                Update,
                gravity_system.run_if(in_state(GameState::Game)),
            );
    }
}

/// Build collision resources from the loaded map data.
fn setup_collision_data(mut commands: Commands, prepared: Option<Res<PreparedWorld>>) {
    let Some(prepared) = &prepared else {
        return;
    };

    commands.insert_resource(TerrainHeightMap {
        heights: prepared.map.height_map.to_vec(),
    });

    // Build collision geometry from BSP model faces
    let mut walls = Vec::new();
    let mut floors = Vec::new();
    for model in &prepared.map.bsp_models {
        for face in &model.faces {
            if face.vertices_count < 3 || face.is_invisible() {
                continue;
            }
            // Face normal: MM6 (x,y,z) → Bevy (x,z,-y)
            let nx = face.plane.normal[0] as f32 / 65536.0;
            let ny = face.plane.normal[2] as f32 / 65536.0;
            let nz = -face.plane.normal[1] as f32 / 65536.0;
            let normal = Vec3::new(nx, ny, nz);

            let is_floor = ny > 0.5;
            let is_wall = ny.abs() < 0.7;

            // Collect face vertices in Bevy coords
            let vert_count = face.vertices_count as usize;
            let verts: Vec<Vec3> = (0..vert_count)
                .filter_map(|i| {
                    let idx = face.vertices_ids[i] as usize;
                    if idx < model.vertices.len() {
                        Some(Vec3::from(model.vertices[idx]))
                    } else {
                        None
                    }
                })
                .collect();
            if verts.len() < 3 {
                continue;
            }

            // Walls: store as plane + polygon (no triangulation needed)
            if is_wall {
                let plane_dist = normal.dot(verts[0]);
                walls.push(CollisionWall::new(normal, plane_dist, &verts));
            }

            // Floors: triangulate for height sampling (needs barycentric interpolation)
            if is_floor {
                for i in 0..verts.len().saturating_sub(2) {
                    floors.push(CollisionTriangle::new(
                        verts[0], verts[i + 1], verts[i + 2], normal,
                    ));
                }
            }
        }
    }
    commands.insert_resource(BuildingColliders { walls, floors });

    // Water map
    commands.insert_resource(WaterMap {
        cells: prepared.water_cells.clone(),
    });
    commands.init_resource::<WaterWalking>();
}

/// Maximum terrain slope angle (radians) the player can stand on.
/// ~35 degrees — anything steeper slides the player downhill.
const MAX_SLOPE_ANGLE: f32 = 0.6;
/// How fast the player slides down steep slopes.
const SLOPE_SLIDE_SPEED: f32 = 4000.0;
/// Sample offset for terrain gradient calculation.
const SLOPE_SAMPLE_DIST: f32 = 32.0;

/// Apply gravity, ground clamping, slope sliding, and fly mode vertical behavior.
fn gravity_system(
    time: Res<Time>,
    height_map: Option<Res<TerrainHeightMap>>,
    colliders: Option<Res<BuildingColliders>>,
    settings: Res<PlayerSettings>,
    fly_mode: Res<FlyMode>,
    mut query: Query<(&mut Transform, &mut PlayerPhysics), With<Player>>,
) {
    let Some(height_map) = height_map else {
        return;
    };

    let dt = time.delta_secs();

    for (mut transform, mut physics) in query.iter_mut() {
        let feet_y = transform.translation.y - settings.eye_height;
        let ground_y = ground_height_at(
            &height_map.heights,
            colliders.as_deref(),
            transform.translation.x,
            transform.translation.z,
            feet_y,
        ) + settings.eye_height;

        if fly_mode.0 {
            physics.vertical_velocity = 0.0;
            physics.on_ground = false;
            if transform.translation.y < ground_y {
                transform.translation.y = ground_y;
            }
        } else {
            physics.vertical_velocity -= settings.gravity * dt;
            transform.translation.y += physics.vertical_velocity * dt;

            if transform.translation.y < ground_y {
                transform.translation.y = ground_y;
                physics.vertical_velocity = 0.0;
                physics.on_ground = true;
            } else if transform.translation.y - ground_y < 2.0 {
                transform.translation.y = ground_y;
                physics.vertical_velocity = 0.0;
                physics.on_ground = true;
            } else {
                physics.on_ground = false;
            }

            // Slope sliding: when on the ground on steep terrain, push downhill.
            if physics.on_ground {
                let px = transform.translation.x;
                let pz = transform.translation.z;
                // Sample terrain gradient using central differences
                let h_xp = sample_terrain_height(&height_map.heights, px + SLOPE_SAMPLE_DIST, pz);
                let h_xn = sample_terrain_height(&height_map.heights, px - SLOPE_SAMPLE_DIST, pz);
                let h_zp = sample_terrain_height(&height_map.heights, px, pz + SLOPE_SAMPLE_DIST);
                let h_zn = sample_terrain_height(&height_map.heights, px, pz - SLOPE_SAMPLE_DIST);

                let grad_x = (h_xp - h_xn) / (2.0 * SLOPE_SAMPLE_DIST);
                let grad_z = (h_zp - h_zn) / (2.0 * SLOPE_SAMPLE_DIST);
                let slope = (grad_x * grad_x + grad_z * grad_z).sqrt();

                if slope > MAX_SLOPE_ANGLE.tan() {
                    // Push downhill (opposite to gradient = uphill direction)
                    let slide_strength = (slope - MAX_SLOPE_ANGLE.tan()) * SLOPE_SLIDE_SPEED * dt;
                    let grad_len = slope.max(0.001);
                    transform.translation.x -= (grad_x / grad_len) * slide_strength;
                    transform.translation.z -= (grad_z / grad_len) * slide_strength;
                }
            }
        }
    }
}
