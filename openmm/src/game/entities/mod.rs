use bevy::prelude::*;

use crate::GameState;
use crate::game::player::PlayerCamera;

pub mod decoration;
pub mod npc;

// Future modules:
// pub mod monster;
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

/// Directional sprite: the displayed frame depends on the angle between
/// the entity's facing direction and the camera. Used by NPCs and monsters.
/// Not implemented yet — placeholder for the sprite angle system.
#[derive(Component)]
pub struct DirectionalSprite {
    /// The entity's facing direction in world space (radians, Y-axis rotation).
    pub facing: f32,
    /// Number of directional frames (typically 8 for MM6).
    pub direction_count: u8,
}

/// Animation state for entities that have multiple frames.
/// Not implemented yet — placeholder for the animation system.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub enum AnimationState {
    Idle,
    Walking,
    Attacking,
    GettingHit,
    Dying,
    Dead,
}

impl Default for AnimationState {
    fn default() -> Self {
        Self::Idle
    }
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
                wander_system,
                npc::update_actor_sprites,
                billboard_face_camera,
            )
                .chain()
                .run_if(in_state(GameState::Game)),
        );
    }
}

/// Simple wander AI: actors pick a random point within tether distance
/// and slowly walk toward it, then pick a new target.
fn wander_system(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut npc::Actor, &mut AnimationState), With<WorldEntity>>,
) {
    let dt = time.delta_secs();

    for (mut transform, mut actor, mut anim_state) in query.iter_mut() {
        if actor.tether_distance < 1.0 && actor.move_speed < 1.0 {
            continue;
        }

        actor.wander_timer -= dt;
        if actor.wander_timer <= 0.0 {
            let angle = (time.elapsed_secs() * 137.5 + actor.initial_position.x)
                % std::f32::consts::TAU;
            let dist = actor.tether_distance.max(200.0) * 0.5;
            actor.wander_target = actor.guarding_position
                + Vec3::new(angle.cos() * dist, 0.0, angle.sin() * dist);
            actor.wander_timer = 3.0 + (angle * 2.0).sin().abs() * 4.0;
        }

        let dir = actor.wander_target - transform.translation;
        let flat_dir = Vec3::new(dir.x, 0.0, dir.z);
        if flat_dir.length() > 10.0 {
            let speed = actor.move_speed.min(80.0) * dt;
            let move_vec = flat_dir.normalize() * speed;
            transform.translation.x += move_vec.x;
            transform.translation.z += move_vec.z;

            // Face movement direction
            let face_angle = move_vec.x.atan2(move_vec.z);
            transform.rotation = Quat::from_rotation_y(face_angle);

            *anim_state = AnimationState::Walking;
        } else {
            *anim_state = AnimationState::Idle;
        }
    }
}

/// Rotate all billboard entities to face the camera (Y-axis only, stays upright).
fn billboard_face_camera(
    camera_query: Query<&GlobalTransform, With<PlayerCamera>>,
    mut billboard_query: Query<(&mut Transform, &GlobalTransform), With<Billboard>>,
) {
    let Ok(camera_gt) = camera_query.single() else {
        return;
    };
    let cam_pos = camera_gt.translation();

    for (mut transform, global_transform) in billboard_query.iter_mut() {
        let bb_pos = global_transform.translation();
        let dir = cam_pos - bb_pos;
        if dir.x.abs() > 0.01 || dir.z.abs() > 0.01 {
            let angle = dir.x.atan2(dir.z);
            transform.rotation = Quat::from_rotation_y(angle);
        }
    }
}
