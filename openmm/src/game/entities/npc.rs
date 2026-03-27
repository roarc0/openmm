use bevy::{asset::RenderAssetUsages, prelude::*};

use lod::LodManager;

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

/// Sprite prefixes for peasant variants. Will be replaced by proper monster table lookup.
const PEASANT_SPRITES: &[&str] = &["pfem", "pman", "pmn2"];

/// Load the standing front sprite for an NPC.
fn load_npc_sprite(
    lod_manager: &LodManager,
    actor_index: usize,
) -> Option<(Image, f32, f32)> {
    let prefix = PEASANT_SPRITES[actor_index % PEASANT_SPRITES.len()];
    let sprite_name = format!("{}sta0", prefix);

    let img = lod_manager.sprite(&sprite_name)?;
    let w = img.width() as f32;
    let h = img.height() as f32;
    let bevy_img = Image::from_dynamic(img, true, RenderAssetUsages::RENDER_WORLD);
    Some((bevy_img, w, h))
}

/// Spawn actors (NPCs/monsters) from the DDM data with sprite textures.
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

        let (sprite_img, sprite_w, sprite_h) = match load_npc_sprite(lod_manager, i) {
            Some(s) => s,
            None => continue,
        };

        let tex_handle = images.add(sprite_img);
        let mat = materials.add(StandardMaterial {
            base_color_texture: Some(tex_handle),
            alpha_mode: AlphaMode::Mask(0.5),
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        });

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
            MeshMaterial3d(mat),
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
