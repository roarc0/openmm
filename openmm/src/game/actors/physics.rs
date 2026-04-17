//! Physics rules shared by all actors (live and dead): gravity, terrain snapping,
//! and movement passability checks.
//!
//! Separating this from `monster_ai` keeps navigation logic (steering, wander targets)
//! decoupled from physical constraints (water, slope, gravity).

use bevy::prelude::*;

use crate::GameState;
use crate::game::actors::Actor;
use crate::game::collision::{BuildingColliders, MAX_STEP_UP, TerrainHeightMap, WaterMap, sample_terrain_height};
use crate::game::state::ui_state::{UiMode, UiState};

/// Gravity acceleration for actors (world units/sec²). Matches player gravity.
pub const ACTOR_GRAVITY: f32 = 9800.0;

/// Max terrain height gain per movement step for grounded actors.
/// Flying actors are exempt — they can ascend freely.
pub const ACTOR_MAX_CLIMB: f32 = MAX_STEP_UP;

pub struct ActorPhysicsPlugin;

impl Plugin for ActorPhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            actor_gravity_system
                .run_if(in_state(GameState::Game))
                .run_if(|ui: Res<UiState>| ui.mode == UiMode::World),
        );
    }
}

/// Snap actor Y to the terrain/BSP floor surface after XZ movement.
///
/// Flying actors hover at `ground + sprite_half_height * 4`. Grounded actors
/// stand at `ground + sprite_half_height`.
///
/// Key invariant: if an actor is elevated on a BSP floor (balcony, rooftop) and
/// moves to an XZ position with no BSP floor, we keep their current Y instead of
/// dropping them to terrain. They only fall to terrain when already near ground level
/// (within MAX_STEP_UP).
pub fn snap_actor_y(
    pos: Vec3,
    sprite_half_height: f32,
    can_fly: bool,
    hm: Option<&TerrainHeightMap>,
    c: Option<&BuildingColliders>,
) -> f32 {
    let feet_y = pos.y - sprite_half_height;
    let terrain_h = hm
        .map(|t| sample_terrain_height(&t.heights, pos.x, pos.z))
        .unwrap_or(f32::MIN);
    let bsp_h = c
        .and_then(|col| col.floor_height_at(pos.x, pos.z, feet_y, MAX_STEP_UP))
        .unwrap_or(f32::MIN);

    let ground = if bsp_h > f32::MIN {
        terrain_h.max(bsp_h)
    } else if terrain_h > f32::MIN {
        if terrain_h >= feet_y - MAX_STEP_UP {
            terrain_h
        } else {
            return pos.y; // elevated on BSP — don't drop to terrain
        }
    } else {
        return pos.y; // indoor, no BSP hit — keep current Y
    };

    if can_fly {
        ground + sprite_half_height * 4.0
    } else {
        ground + sprite_half_height
    }
}

/// Check whether an actor can move from `from_y` to `dest` at height `dest_y`.
/// Rejects water tiles and terrain climbs too steep for grounded actors.
/// Flying actors are exempt from the slope check.
pub fn is_passable(actor: &Actor, from_y: f32, dest: Vec3, dest_y: f32, wm: Option<&WaterMap>) -> bool {
    if wm.is_some_and(|w| w.is_water_at(dest.x, dest.z)) {
        return false;
    }
    if !actor.can_fly && dest_y - from_y > ACTOR_MAX_CLIMB {
        return false;
    }
    true
}

/// Apply gravity to all actors. Live flying actors are exempt (AI manages their Y).
/// Dead flying actors lose lift and fall like everything else.
fn actor_gravity_system(
    time: Res<Time>,
    height_map: Option<Res<TerrainHeightMap>>,
    colliders: Option<Res<BuildingColliders>>,
    mut query: Query<(&mut Transform, &mut Actor)>,
) {
    if height_map.is_none() && colliders.is_none() {
        return;
    }

    let dt = time.delta_secs();

    for (mut transform, mut actor) in query.iter_mut() {
        if actor.can_fly && actor.hp > 0 {
            continue;
        }

        let feet_y = transform.translation.y - actor.sprite_half_height;
        let terrain_h = height_map
            .as_deref()
            .map(|hm| sample_terrain_height(&hm.heights, transform.translation.x, transform.translation.z))
            .unwrap_or(f32::MIN);
        let bsp_floor_h = colliders
            .as_deref()
            .and_then(|c| c.floor_height_at(transform.translation.x, transform.translation.z, feet_y, MAX_STEP_UP))
            .unwrap_or(f32::MIN);
        let ground_y = terrain_h.max(bsp_floor_h) + actor.sprite_half_height;

        actor.vertical_velocity -= ACTOR_GRAVITY * dt;
        transform.translation.y += actor.vertical_velocity * dt;

        if transform.translation.y <= ground_y {
            transform.translation.y = ground_y;
            actor.vertical_velocity = 0.0;
        }
    }
}
