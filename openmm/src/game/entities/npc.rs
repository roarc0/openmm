use bevy::prelude::*;

use lod::ddm::DdmActor;

use crate::game::entities::{AnimationState, Billboard, EntityKind, WorldEntity};
use crate::states::loading::PreparedWorld;

/// NPC/actor-specific component with runtime data.
#[derive(Component)]
pub struct Actor {
    pub name: String,
    pub hp: i16,
    pub move_speed: f32,
    pub initial_position: Vec3,
    pub guarding_position: Vec3,
    pub tether_distance: f32,
    pub wander_timer: f32,
    pub wander_target: Vec3,
}

/// Spawn actors (NPCs/monsters) from the DDM data as placeholder billboards.
pub fn spawn_actors(
    parent: &mut ChildSpawnerCommands,
    prepared: &PreparedWorld,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let npc_color = Color::srgb(0.3, 0.7, 0.3);
    let npc_mat = materials.add(StandardMaterial {
        base_color: npc_color,
        alpha_mode: AlphaMode::Opaque,
        unlit: true,
        double_sided: true,
        cull_mode: None,
        ..default()
    });

    let quad = meshes.add(Rectangle::new(60.0, 140.0));

    for actor in &prepared.actors {
        if actor.hp <= 0 {
            continue; // Skip dead actors
        }

        // MM6 coords (x, y, z) → Bevy (x, z, -y)
        let pos = Vec3::new(
            actor.position[0] as f32,
            actor.position[2] as f32 + 70.0, // offset up by half height
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
            Mesh3d(quad.clone()),
            MeshMaterial3d(npc_mat.clone()),
            Transform::from_translation(pos),
            WorldEntity,
            EntityKind::Npc,
            Billboard,
            AnimationState::Idle,
            Actor {
                name: actor.name.clone(),
                hp: actor.hp,
                move_speed: actor.move_speed as f32,
                initial_position: initial,
                guarding_position: guarding,
                tether_distance: actor.tether_distance as f32,
                wander_timer: 0.0,
                wander_target: pos,
            },
        ));
    }
}
