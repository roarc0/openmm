use bevy::prelude::*;

use crate::GameState;
use crate::game::map::collision::{BuildingColliders, TerrainHeightMap, sample_terrain_height};
use crate::game::player::{Player, PlayerPhysics, PlayerSettings};

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        // Collision resources (BuildingColliders, TerrainHeightMap, WaterMap)
        // are built during the loading pipeline so they're available at
        // spawn time for probe_ground_height (bridges, BSP floors).
        app.add_systems(
            Update,
            gravity_system
                .run_if(in_state(GameState::Game))
                .run_if(crate::game::ui::is_world_mode),
        );
    }
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
    world_state: Res<crate::game::state::WorldState>,
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
                    crate::game::map::collision::MAX_STEP_UP,
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
