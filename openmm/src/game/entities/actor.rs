//! Actor entity: NPCs and monsters. Both use the same underlying struct —
//! the difference is the `hostile` flag.

use bevy::prelude::*;

use lod::{dsft::DSFT, LodManager};

use crate::game::entities::{AnimationState, EntityKind, WorldEntity, sprites};
use crate::states::loading::PreparedWorld;

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

/// Spawn actors from DDM data with sprites resolved via DSFT.
pub fn spawn_actors(
    parent: &mut ChildSpawnerCommands,
    prepared: &PreparedWorld,
    lod_manager: &LodManager,
    images: &mut ResMut<Assets<Image>>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let dsft = match DSFT::new(lod_manager) {
        Ok(d) => d,
        Err(_) => return,
    };

    for (_i, actor) in prepared.actors.iter().enumerate() {
        if actor.hp <= 0 {
            continue;
        }
        if actor.position[0].abs() > 20000 || actor.position[1].abs() > 20000 {
            continue;
        }

        let (sprite_sheet, sprite_w, sprite_h) = match sprites::load_actor_sprite_sheet(
            &dsft,
            &actor.sprite_ids,
            lod_manager,
            images,
            materials,
        ) {
            Some(s) => s,
            None => continue,
        };

        let initial_mat = sprite_sheet.states[0][0][0].clone();
        let quad = meshes.add(Rectangle::new(sprite_w, sprite_h));

        // MM6 coords (x, y, z) → Bevy (x, z, -y)
        let pos = Vec3::new(
            actor.position[0] as f32,
            actor.position[2] as f32 + sprite_h / 2.0,
            -actor.position[1] as f32,
        );
        let initial = Vec3::new(
            actor.initial_position[0] as f32,
            actor.initial_position[2] as f32,
            -actor.initial_position[1] as f32,
        );
        let guarding = Vec3::new(
            actor.guarding_position[0] as f32,
            actor.guarding_position[2] as f32,
            -actor.guarding_position[1] as f32,
        );

        parent.spawn((
            Name::new(format!("actor:{}", actor.name)),
            Mesh3d(quad),
            MeshMaterial3d(initial_mat),
            Transform::from_translation(pos),
            WorldEntity,
            EntityKind::Npc,
            AnimationState::Idle,
            sprite_sheet,
            Actor {
                name: actor.name.clone(),
                hp: actor.hp,
                max_hp: actor.hp,
                move_speed: actor.move_speed as f32,
                initial_position: initial,
                guarding_position: guarding,
                tether_distance: actor.tether_distance as f32,
                wander_timer: 0.0,
                wander_target: pos,
                facing_yaw: 0.0,
                hostile: false,
            },
        ));
    }
}
