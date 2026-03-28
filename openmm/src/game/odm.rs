use bevy::prelude::*;
use bevy::render::render_resource::Face;

use crate::GameState;
use crate::game::InGame;
use crate::assets::GameAssets;
use crate::game::entities::{actor, decoration, sprites};
use crate::game::player::Player;

/// Pending entities to spawn lazily as the player moves.
#[derive(Resource)]
struct PendingSpawns {
    /// Index of next billboard to check.
    billboard_idx: usize,
    /// Index of next actor to check.
    actor_idx: usize,
    /// Index of next monster to check.
    monster_idx: usize,
    /// Shared sprite cache.
    sprite_cache: sprites::SpriteCache,
    /// Terrain entity to parent everything under.
    terrain_entity: Entity,
}
use crate::states::loading::PreparedWorld;

/// Grid coordinate for outdoor maps. Columns a-e, rows 1-3.
#[derive(Clone)]
pub struct OdmName {
    pub x: char,
    pub y: char,
}

impl Default for OdmName {
    fn default() -> Self {
        Self { x: 'e', y: '3' }
    }
}

use std::fmt::Display;

impl Display for OdmName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(Self::map_name(self.x, self.y).as_str())
    }
}

impl TryFrom<&str> for OdmName {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let x = value.as_bytes().get(3).copied().ok_or("invalid map name")? as char;
        let y = value.as_bytes().get(4).copied().ok_or("invalid map name")? as char;

        let x = Self::validate_x(x).ok_or("invalid map x coordinate")?;
        let y = Self::validate_y(y).ok_or("invalid map y coordinate")?;

        Ok(Self { x, y })
    }
}

impl OdmName {
    pub fn go_north(&self) -> Option<OdmName> {
        let y = Self::validate_y((self.y as u8 - 1) as char)?;
        Some(Self { x: self.x, y })
    }

    pub fn go_west(&self) -> Option<OdmName> {
        let x = Self::validate_x((self.x as u8 - 1) as char)?;
        Some(Self { x, y: self.y })
    }

    pub fn go_south(&self) -> Option<OdmName> {
        let y = Self::validate_y((self.y as u8 + 1) as char)?;
        Some(Self { x: self.x, y })
    }

    pub fn go_east(&self) -> Option<OdmName> {
        let x = Self::validate_x((self.x as u8 + 1) as char)?;
        Some(Self { x, y: self.y })
    }

    fn map_name(x: char, y: char) -> String {
        format!("out{}{}.odm", x, y)
    }

    fn validate_x(c: char) -> Option<char> {
        match c {
            'a'..='e' => Some(c),
            _ => None,
        }
    }
    fn validate_y(c: char) -> Option<char> {
        match c {
            '1'..='3' => Some(c),
            _ => None,
        }
    }
}

const SPAWN_RADIUS: f32 = 15000.0;
const SPAWN_BATCH_SIZE: usize = 10;

pub struct OdmPlugin;

impl Plugin for OdmPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Game), spawn_world)
            .add_systems(Update, lazy_spawn.run_if(in_state(GameState::Game)));
    }
}

fn spawn_world(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    prepared: Option<Res<PreparedWorld>>,
    game_assets: Res<GameAssets>,
) {
    let Some(prepared) = prepared else {
        error!("No PreparedWorld available when entering Game state");
        return;
    };

    let terrain_texture = prepared.terrain_texture.clone();
    let terrain_mesh = prepared.terrain_mesh.clone();

    let image_handle = images.add(terrain_texture);
    let material = StandardMaterial {
        base_color: Color::srgb(0.85, 0.85, 0.85),
        base_color_texture: Some(image_handle),
        unlit: false,
        alpha_mode: AlphaMode::Opaque,
        perceptual_roughness: 1.0,
        reflectance: 0.0,
        metallic: 0.0,
        cull_mode: Some(Face::Back),
        double_sided: false,
        ..default()
    };

    let terrain_entity = commands
        .spawn((
            Name::new("odm"),
            Mesh3d(meshes.add(terrain_mesh)),
            MeshMaterial3d(materials.add(material)),
            InGame,
        ))
        .with_children(|parent| {
            // BSP models (buildings, structures)
            for model in &prepared.models {
                for sub in &model.sub_meshes {
                    let mut mat = sub.material.clone();
                    if let Some(ref tex) = sub.texture {
                        let tex_handle = images.add(tex.clone());
                        mat.base_color_texture = Some(tex_handle);
                    }
                    parent.spawn((
                        Name::new("model"),
                        Mesh3d(meshes.add(sub.mesh.clone())),
                        MeshMaterial3d(materials.add(mat)),
                    ));
                }
            }
        }).id();

    // Set up lazy spawning — entities spawn gradually each frame
    commands.insert_resource(PendingSpawns {
        billboard_idx: 0,
        actor_idx: 0,
        monster_idx: 0,
        sprite_cache: prepared.sprite_cache.clone(),
        terrain_entity,
    });
}
/// Spawn entities gradually each frame near the player.
fn lazy_spawn(
    mut commands: Commands,
    mut pending: Option<ResMut<PendingSpawns>>,
    prepared: Option<Res<PreparedWorld>>,
    game_assets: Res<GameAssets>,
    player_query: Query<&GlobalTransform, With<Player>>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (Some(mut pending), Some(prepared)) = (pending, prepared) else {
        return;
    };
    let player_pos = player_query.single().map(|t| t.translation()).unwrap_or(Vec3::ZERO);

    let mut spawned = 0;

    // Spawn billboards
    while pending.billboard_idx < prepared.billboards.len() && spawned < SPAWN_BATCH_SIZE {
        let bb = &prepared.billboards[pending.billboard_idx];
        pending.billboard_idx += 1;

        let dist_sq = player_pos.distance_squared(bb.position);
        if dist_sq > SPAWN_RADIUS * SPAWN_RADIUS {
            continue; // Skip far billboards for now, we'll get them on next pass
        }

        let tex_handle = images.add(bb.image.clone());
        let bb_mat = materials.add(StandardMaterial {
            base_color_texture: Some(tex_handle),
            alpha_mode: AlphaMode::Mask(0.5),
            cull_mode: None,
            double_sided: true,
            unlit: true,
            ..default()
        });
        let quad = meshes.add(Rectangle::new(bb.width, bb.height));
        let pos = bb.position + Vec3::new(0.0, bb.height / 2.0, 0.0);

        commands.entity(pending.terrain_entity).with_child((
            Name::new("decoration"),
            Mesh3d(quad),
            MeshMaterial3d(bb_mat),
            Transform::from_translation(pos),
            crate::game::entities::WorldEntity,
            crate::game::entities::EntityKind::Decoration,
            crate::game::entities::Billboard,
        ));
        spawned += 1;
    }

    // Spawn NPC actors
    let npc_sprites = [("pfemst", "pfemwa"), ("pmanst", "pmanwk"), ("pmn2st", "pmn2wa")];
    while pending.actor_idx < prepared.actors.len() && spawned < SPAWN_BATCH_SIZE {
        let i = pending.actor_idx;
        let ddm_actor = &prepared.actors[i];
        pending.actor_idx += 1;

        if ddm_actor.hp <= 0 || ddm_actor.position[0].abs() > 20000 || ddm_actor.position[1].abs() > 20000 {
            continue;
        }

        let (st_root, wa_root) = npc_sprites[i % npc_sprites.len()];
        let (standing, sw, sh) = sprites::load_sprite_frames_cached(
            st_root, game_assets.lod_manager(), &mut images, &mut materials, &mut Some(&mut pending.sprite_cache));
        if standing.is_empty() { continue; }
        let (walking, _, _) = sprites::load_sprite_frames_cached(
            wa_root, game_assets.lod_manager(), &mut images, &mut materials, &mut Some(&mut pending.sprite_cache));

        let mut states = vec![standing];
        if !walking.is_empty() { states.push(walking); }

        let initial_mat = states[0][0][0].clone();
        let quad = meshes.add(Rectangle::new(sw, sh));

        let world_x = ddm_actor.position[0] as f32;
        let world_z = -ddm_actor.position[1] as f32;
        let ground_y = crate::game::collision::probe_ground_height(&prepared.map.height_map[..], None, world_x, world_z);
        let pos = Vec3::new(world_x, ground_y + sh / 2.0, world_z);

        commands.entity(pending.terrain_entity).with_child((
            Name::new(format!("npc:{}", ddm_actor.name)),
            Mesh3d(quad),
            MeshMaterial3d(initial_mat),
            Transform::from_translation(pos),
            crate::game::entities::WorldEntity,
            crate::game::entities::EntityKind::Npc,
            crate::game::entities::AnimationState::Idle,
            sprites::SpriteSheet { states, current_frame: 0, frame_timer: 0.0, frame_duration: 0.15 },
            actor::Actor {
                name: ddm_actor.name.clone(), hp: ddm_actor.hp, max_hp: ddm_actor.hp,
                move_speed: ddm_actor.move_speed as f32,
                initial_position: pos, guarding_position: pos,
                tether_distance: ddm_actor.tether_distance as f32,
                wander_timer: 0.0, wander_target: pos, facing_yaw: 0.0, hostile: false,
            },
        ));
        spawned += 1;
    }

    // Spawn monsters
    while pending.monster_idx < prepared.monsters.len() && spawned < SPAWN_BATCH_SIZE {
        let monster = &prepared.monsters[pending.monster_idx];
        pending.monster_idx += 1;

        let fallbacks = [
            (monster.standing_sprite.as_str(), monster.walking_sprite.as_str()),
            ("pfemst", "pfemwa"), ("pmanst", "pmanwk"),
        ];
        let mut standing = Vec::new();
        let mut walking = Vec::new();
        let mut sw = 0.0f32;
        let mut sh = 0.0f32;
        for (st, wa) in &fallbacks {
            let (sf, w, h) = sprites::load_sprite_frames_cached(
                st, game_assets.lod_manager(), &mut images, &mut materials, &mut Some(&mut pending.sprite_cache));
            if !sf.is_empty() && w > 0.0 {
                standing = sf; sw = w; sh = h;
                let (wf, _, _) = sprites::load_sprite_frames_cached(
                    wa, game_assets.lod_manager(), &mut images, &mut materials, &mut Some(&mut pending.sprite_cache));
                walking = wf;
                break;
            }
        }
        if standing.is_empty() { continue; }

        let mut states = vec![standing];
        if !walking.is_empty() { states.push(walking); }

        let initial_mat = states[0][0][0].clone();
        let quad = meshes.add(Rectangle::new(sw, sh));

        let world_x = monster.position[0] as f32;
        let world_z = -monster.position[1] as f32;
        let ground_y = crate::game::collision::probe_ground_height(&prepared.map.height_map[..], None, world_x, world_z);
        let pos = Vec3::new(world_x, ground_y + sh / 2.0, world_z);

        commands.entity(pending.terrain_entity).with_child((
            Name::new("monster"),
            Mesh3d(quad),
            MeshMaterial3d(initial_mat),
            Transform::from_translation(pos),
            crate::game::entities::WorldEntity,
            crate::game::entities::EntityKind::Monster,
            crate::game::entities::AnimationState::Idle,
            sprites::SpriteSheet { states, current_frame: 0, frame_timer: 0.0, frame_duration: 0.15 },
            actor::Actor {
                name: "Monster".into(), hp: 10, max_hp: 10,
                move_speed: monster.move_speed as f32,
                initial_position: pos, guarding_position: pos,
                tether_distance: monster.radius.max(200) as f32,
                wander_timer: 0.0, wander_target: pos, facing_yaw: 0.0, hostile: true,
            },
        ));
        spawned += 1;
    }

    // If everything is spawned, remove the resource
    if pending.billboard_idx >= prepared.billboards.len()
        && pending.actor_idx >= prepared.actors.len()
        && pending.monster_idx >= prepared.monsters.len()
    {
        commands.remove_resource::<PendingSpawns>();
    }
}
