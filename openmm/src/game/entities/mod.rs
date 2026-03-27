use bevy::prelude::*;

use crate::GameState;
use crate::game::player::PlayerCamera;

pub mod decoration;

// Future modules:
// pub mod monster;
// pub mod npc;
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
            billboard_face_camera.run_if(in_state(GameState::Game)),
        );
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
