use bevy::prelude::*;

use lod::enums::PolygonType;

use crate::GameState;
use crate::game::collision::{
    BuildingColliders, CollisionTriangle, CollisionWall, TerrainHeightMap, WaterMap, WaterWalking,
    sample_terrain_height,
};
use crate::game::player::{Player, PlayerPhysics, PlayerSettings};
use crate::states::loading::{PreparedIndoorWorld, PreparedWorld};

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Game), setup_collision_data)
            .add_systems(
                Update,
                gravity_system
                    .run_if(in_state(GameState::Game))
                    .run_if(resource_equals(crate::game::hud::HudView::World)),
            );
    }
}

/// Build collision resources from the loaded map data.
fn setup_collision_data(
    mut commands: Commands,
    prepared: Option<Res<PreparedWorld>>,
    indoor: Option<Res<PreparedIndoorWorld>>,
) {
    if let Some(indoor) = &indoor {
        // Indoor map: collision from pre-extracted BLV faces
        let mut colliders = BuildingColliders {
            walls: indoor.collision_walls.clone(),
            floors: indoor.collision_floors.clone(),
            ceilings: indoor.collision_ceilings.clone(),
        };
        colliders.mark_step_walls();
        commands.insert_resource(colliders);
        return;
    }

    let Some(prepared) = &prepared else {
        return;
    };

    commands.insert_resource(TerrainHeightMap {
        heights: prepared.map.height_map.to_vec(),
    });

    // Build collision geometry from BSP model faces
    let mut walls = Vec::new();
    let mut floors = Vec::new();
    let mut ceilings = Vec::new();
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

            // Use the authoritative polygon_type from game data to classify faces.
            // InBetweenFloorAndWall (stairs/ramps) is treated as walkable floor so
            // floor_height_at can interpolate height across the slope surface.
            let poly_type = face.polygon_type_enum();
            let is_floor = matches!(
                poly_type,
                Some(PolygonType::Floor) | Some(PolygonType::InBetweenFloorAndWall)
            );
            let is_ceiling = matches!(
                poly_type,
                Some(PolygonType::Ceiling) | Some(PolygonType::InBetweenCeilingAndWall)
            );
            let is_wall = matches!(poly_type, Some(PolygonType::VerticalWall));

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
    }
    let mut colliders = BuildingColliders {
        walls,
        floors,
        ceilings,
    };
    colliders.mark_step_walls();
    commands.insert_resource(colliders);

    // Water map (outdoor only)
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

fn gravity_system(
    time: Res<Time>,
    height_map: Option<Res<TerrainHeightMap>>,
    colliders: Option<Res<BuildingColliders>>,
    settings: Res<PlayerSettings>,
    world_state: Res<crate::game::world_state::WorldState>,
    mut query: Query<(&mut Transform, &mut PlayerPhysics), With<Player>>,
) {
    // Need at least one ground source
    if height_map.is_none() && colliders.is_none() {
        return;
    }

    let dt = time.delta_secs();

    for (mut transform, mut physics) in query.iter_mut() {
        let feet_y = transform.translation.y - settings.eye_height;

        let terrain_h = height_map
            .as_deref()
            .map(|hm| sample_terrain_height(&hm.heights, transform.translation.x, transform.translation.z))
            .unwrap_or(f32::MIN);
        let bsp_floor_h = colliders
            .as_deref()
            .and_then(|c| {
                c.floor_height_at(
                    transform.translation.x,
                    transform.translation.z,
                    feet_y,
                    crate::game::collision::MAX_STEP_UP,
                )
            })
            .unwrap_or(f32::MIN);
        let ground_y = terrain_h.max(bsp_floor_h) + settings.eye_height;

        // Ceiling clamp: prevent player from going above the lowest ceiling
        let ceiling_y = colliders
            .as_deref()
            .and_then(|c| {
                c.ceiling_height_at(
                    transform.translation.x,
                    transform.translation.z,
                    transform.translation.y,
                    feet_y,
                )
            })
            .unwrap_or(f32::MAX);

        if world_state.player.fly_mode {
            physics.vertical_velocity = 0.0;
            physics.on_ground = false;
            if transform.translation.y < ground_y {
                transform.translation.y = ground_y;
                physics.on_ground = true;
            }
            if transform.translation.y > ceiling_y - settings.eye_height {
                transform.translation.y = ceiling_y - settings.eye_height;
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

            // Ceiling clamp
            if transform.translation.y > ceiling_y - settings.eye_height {
                transform.translation.y = ceiling_y - settings.eye_height;
                physics.vertical_velocity = 0.0;
            }

            // Slope sliding: only on outdoor terrain
            if physics.on_ground
                && let Some(ref hm) = height_map
            {
                let px = transform.translation.x;
                let pz = transform.translation.z;
                let h_xp = sample_terrain_height(&hm.heights, px + SLOPE_SAMPLE_DIST, pz);
                let h_xn = sample_terrain_height(&hm.heights, px - SLOPE_SAMPLE_DIST, pz);
                let h_zp = sample_terrain_height(&hm.heights, px, pz + SLOPE_SAMPLE_DIST);
                let h_zn = sample_terrain_height(&hm.heights, px, pz - SLOPE_SAMPLE_DIST);

                let grad_x = (h_xp - h_xn) / (2.0 * SLOPE_SAMPLE_DIST);
                let grad_z = (h_zp - h_zn) / (2.0 * SLOPE_SAMPLE_DIST);
                let slope = (grad_x * grad_x + grad_z * grad_z).sqrt();

                if slope > MAX_SLOPE_ANGLE.tan() {
                    let slide_strength = (slope - MAX_SLOPE_ANGLE.tan()) * SLOPE_SLIDE_SPEED * dt;
                    let grad_len = slope.max(0.001);
                    transform.translation.x -= (grad_x / grad_len) * slide_strength;
                    transform.translation.z -= (grad_z / grad_len) * slide_strength;
                }
            }
        }
    }
}
