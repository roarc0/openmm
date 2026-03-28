//! Actor entity: NPCs and monsters.

use bevy::prelude::*;

use lod::LodManager;

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

/// Peasant sprite prefixes — alternated for variety.
const NPC_SPRITES: &[(&str, &str)] = &[
    ("pfemst", "pfemwa"),
    ("pmanst", "pmanwk"),
    ("pmn2st", "pmn2wa"),
];

/// Spawn DDM actors (NPCs) with peasant sprites.
pub fn spawn_actors(
    parent: &mut ChildSpawnerCommands,
    prepared: &PreparedWorld,
    lod_manager: &LodManager,
    images: &mut ResMut<Assets<Image>>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    for (i, actor) in prepared.actors.iter().enumerate() {
        if actor.hp <= 0 {
            continue;
        }
        if actor.position[0].abs() > 20000 || actor.position[1].abs() > 20000 {
            continue;
        }

        // Use peasant sprites, cycling through variants
        let (st_root, wa_root) = NPC_SPRITES[i % NPC_SPRITES.len()];

        let (standing_frames, sprite_w, sprite_h) =
            sprites::load_sprite_frames(st_root, lod_manager, images, materials);
        if standing_frames.is_empty() || sprite_w == 0.0 {
            continue;
        }
        let (walking_frames, _, _) =
            sprites::load_sprite_frames(wa_root, lod_manager, images, materials);

        let mut states = vec![standing_frames];
        if !walking_frames.is_empty() {
            states.push(walking_frames);
        }

        let initial_mat = states[0][0][0].clone();
        let quad = meshes.add(Rectangle::new(sprite_w, sprite_h));

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
            Name::new(format!("npc:{}", actor.name)),
            Mesh3d(quad),
            MeshMaterial3d(initial_mat),
            Transform::from_translation(pos),
            WorldEntity,
            EntityKind::Npc,
            AnimationState::Idle,
            sprites::SpriteSheet {
                states,
                current_frame: 0,
                frame_timer: 0.0,
                frame_duration: 0.15,
            },
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

/// Spawn monsters from ODM spawn points with sprites from dmonlist.
pub fn spawn_monsters(
    parent: &mut ChildSpawnerCommands,
    prepared: &PreparedWorld,
    lod_manager: &LodManager,
    images: &mut ResMut<Assets<Image>>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    info!("Spawning {} monsters from spawn points", prepared.monsters.len());

    for monster in &prepared.monsters {
        // Try monster sprites, fall back to peasant if not found
        let sprite_fallbacks = [
            (monster.standing_sprite.as_str(), monster.walking_sprite.as_str()),
            ("pfemst", "pfemwa"),
            ("pmanst", "pmanwk"),
        ];

        let mut standing_frames = Vec::new();
        let mut walking_frames = Vec::new();
        let mut sprite_w = 0.0_f32;
        let mut sprite_h = 0.0_f32;

        for (st, wa) in &sprite_fallbacks {
            let (sf, w, h) = sprites::load_sprite_frames(st, lod_manager, images, materials);
            if !sf.is_empty() && w > 0.0 {
                standing_frames = sf;
                sprite_w = w;
                sprite_h = h;
                let (wf, _, _) = sprites::load_sprite_frames(wa, lod_manager, images, materials);
                walking_frames = wf;
                break;
            }
        }

        if standing_frames.is_empty() || sprite_w == 0.0 {
            continue;
        }

        let mut states = vec![standing_frames];
        if !walking_frames.is_empty() {
            states.push(walking_frames);
        }

        let initial_mat = states[0][0][0].clone();
        let quad = meshes.add(Rectangle::new(sprite_w, sprite_h));

        // MM6 coords (x, y, z) → Bevy (x, z, -y)
        let pos = Vec3::new(
            monster.position[0] as f32,
            monster.position[2] as f32 + sprite_h / 2.0,
            -monster.position[1] as f32,
        );

        parent.spawn((
            Name::new("monster"),
            Mesh3d(quad),
            MeshMaterial3d(initial_mat),
            Transform::from_translation(pos),
            WorldEntity,
            EntityKind::Monster,
            AnimationState::Idle,
            sprites::SpriteSheet {
                states,
                current_frame: 0,
                frame_timer: 0.0,
                frame_duration: 0.15,
            },
            Actor {
                name: "Monster".into(),
                hp: 10,
                max_hp: 10,
                move_speed: monster.move_speed as f32,
                initial_position: pos,
                guarding_position: pos,
                tether_distance: monster.radius.max(200) as f32,
                wander_timer: 0.0,
                wander_target: pos,
                facing_yaw: 0.0,
                hostile: monster.hostile,
            },
        ));
    }
}
