use bevy::prelude::*;

use crate::GameState;
use crate::game::hud::HudView;

pub mod actor;
pub mod sprites;

// Future modules:
// pub mod loot;

// --- Shared components for all world entities ---

/// All world entities that are spawned from map data.
#[derive(Component)]
pub struct WorldEntity;

/// What kind of world entity this is.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub enum EntityKind {
    /// Static decoration: trees, rocks, fountains, etc. Single sprite, no behavior.
    Decoration,
    /// Interactive NPC: has dialogue, directional sprites, idle animations.
    Npc,
    /// Monster: directional sprites, multiple animation states, can be killed and looted.
    Monster,
    /// Item on the ground: can be picked up.
    Loot,
}

/// Billboard rendering: entity always faces the camera.
#[derive(Component)]
pub struct Billboard;

/// Fixed facing direction in world space (radians, Y-axis rotation).
/// Used by directional decorations (e.g. ships) whose displayed sprite depends
/// on the camera angle relative to this facing. Actor entities use Actor.facing_yaw instead.
#[derive(Component)]
pub struct FacingYaw(pub f32);

/// Animation state for entities that have multiple frames.
/// Not implemented yet — placeholder for the animation system.
#[derive(Component, Debug, Clone, PartialEq, Eq, Default)]
pub enum AnimationState {
    #[default]
    Idle,
    Walking,
    Attacking,
    GettingHit,
    Dying,
    Dead,
}

/// Loot container: when a monster dies, this component is added so it can be looted.
/// Not implemented yet.
#[derive(Component)]
pub struct Lootable;

// --- Plugin ---

pub struct EntitiesPlugin;

impl Plugin for EntitiesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                distance_culling,
                wander_system,
                sprites::update_sprite_sheets,
                billboard_face_camera,
            )
                .chain()
                .run_if(in_state(GameState::Game))
                .run_if(resource_equals(HudView::World)),
        );
    }
}

/// Hide entities that are far from the player, show ones that are close.
fn distance_culling(
    cfg: Res<crate::config::GameConfig>,
    player_query: Query<&GlobalTransform, With<crate::game::player::Player>>,
    mut entity_query: Query<(&GlobalTransform, &mut Visibility), With<WorldEntity>>,
) {
    let Ok(player_gt) = player_query.single() else {
        return;
    };
    let player_pos = player_gt.translation();
    let draw_dist_sq = cfg.draw_distance * cfg.draw_distance;

    for (entity_gt, mut vis) in entity_query.iter_mut() {
        let dist_sq = player_pos.distance_squared(entity_gt.translation());
        let new_vis = if dist_sq < draw_dist_sq {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != new_vis {
            *vis = new_vis;
        }
    }
}

/// Simple wander AI: actors pick a random point within tether distance
/// and slowly walk toward it, then pick a new target.
fn wander_system(
    time: Res<Time>,
    colliders: Option<Res<crate::game::collision::BuildingColliders>>,
    mut query: Query<(&mut Transform, &mut actor::Actor, &mut AnimationState), With<WorldEntity>>,
) {
    let dt = time.delta_secs();

    for (mut transform, mut actor, mut anim_state) in query.iter_mut() {
        if actor.move_speed < 1.0 {
            continue;
        }

        actor.wander_timer -= dt;

        if actor.wander_timer <= 0.0 {
            // Position-based seed keeps each actor on its own independent schedule.
            // Using shared time as seed causes all actors to synchronize and all
            // fire collision checks in the same frame, causing periodic spikes.
            let pos_seed = actor.initial_position.x * 7.3 + actor.initial_position.z * 13.7;

            // Toggle between idle and walking
            if *anim_state == AnimationState::Idle {
                // Pick a new target and start walking
                let seed = pos_seed + time.elapsed_secs() * 0.5;
                let angle = (seed * 2.3).sin() * std::f32::consts::TAU;
                let dist = actor.tether_distance.max(300.0) * 0.4;
                actor.wander_target = actor.guarding_position + Vec3::new(angle.cos() * dist, 0.0, angle.sin() * dist);
                actor.wander_timer = 3.0 + (seed.cos().abs()) * 3.0; // walk for 3-6s
                *anim_state = AnimationState::Walking;
            } else {
                // Stop and idle — seed from position, not from shared elapsed time.
                // Using time.elapsed_secs() here gave all actors the same idle duration
                // when they transitioned in the same frame, causing synchronized wake-ups.
                actor.wander_timer = 2.0 + (pos_seed * 3.7).sin().abs() * 3.0; // idle 2-5s
                *anim_state = AnimationState::Idle;
            }
        }

        // Only move when walking
        if *anim_state == AnimationState::Walking {
            let dir = actor.wander_target - transform.translation;
            let flat_dir = Vec3::new(dir.x, 0.0, dir.z);
            if flat_dir.length() > 20.0 {
                let speed = actor.move_speed.min(60.0) * dt;
                let move_vec = flat_dir.normalize() * speed;

                let from = transform.translation;
                let mut dest = from + Vec3::new(move_vec.x, 0.0, move_vec.z);

                if let Some(ref c) = colliders {
                    dest = c.resolve_movement(from, dest, 20.0, 140.0);
                }

                transform.translation.x = dest.x;
                transform.translation.z = dest.z;

                actor.facing_yaw = move_vec.x.atan2(move_vec.z);
            } else {
                // Reached target, switch to idle
                *anim_state = AnimationState::Idle;
                actor.wander_timer = 2.0;
            }
        }
    }
}

/// Rotate visible billboard entities to face the camera (Y-axis only, stays upright).
/// Only processes visible entities — distance_culling already hides far ones.
/// Skips entities with SpriteSheet (those are handled by update_sprite_sheets).
fn billboard_face_camera(
    camera_query: Query<&GlobalTransform, With<crate::game::player::PlayerCamera>>,
    mut billboard_query: Query<
        (&mut Transform, &GlobalTransform, &Visibility),
        (With<Billboard>, Without<sprites::SpriteSheet>),
    >,
) {
    let Ok(camera_gt) = camera_query.single() else {
        return;
    };
    let cam_pos = camera_gt.translation();

    for (mut transform, global_transform, vis) in billboard_query.iter_mut() {
        if *vis == Visibility::Hidden {
            continue;
        }
        let dir = cam_pos - global_transform.translation();
        if dir.x.abs() > 0.01 || dir.z.abs() > 0.01 {
            transform.rotation = Quat::from_rotation_y(dir.x.atan2(dir.z));
        }
    }
}
