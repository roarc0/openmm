//! Actor entity: NPCs and monsters.

use bevy::prelude::*;

/// Unified NPC/monster actor component.
#[derive(Component)]
pub struct Actor {
    pub name: String,
    pub hp: i16,
    pub max_hp: i16,
    pub move_speed: f32,
    pub initial_position: Vec3,
    pub guarding_position: Vec3,
    pub tether_distance: f32,
    pub wander_timer: f32,
    pub wander_target: Vec3,
    pub facing_yaw: f32,
    pub hostile: bool,
}
