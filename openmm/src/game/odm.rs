use bevy::prelude::*;
use bevy::render::render_resource::Face;

use crate::GameState;
use crate::game::InGame;
use crate::assets::GameAssets;
use crate::game::entities::{actor, decoration, sprites};
use crate::game::player::Player;

/// Pending entities sorted by distance from player, spawned gradually.
#[derive(Resource)]
struct PendingSpawns {
    billboard_order: Vec<usize>,
    actor_order: Vec<usize>,
    monster_order: Vec<usize>,
    idx: usize,
    sprite_cache: sprites::SpriteCache,
    /// Cached billboard materials: key = sprite name, value = (material, mesh, height)
    billboard_cache: std::collections::HashMap<String, (Handle<StandardMaterial>, Handle<Mesh>, f32)>,
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

const SPAWN_BATCH_SIZE: usize = 2;

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
    save_data: Res<crate::save::GameSave>,
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

    // Sort spawn order by distance from player (closest first)
    let player_spawn = Vec3::new(
        save_data.player.position[0],
        save_data.player.position[1],
        save_data.player.position[2],
    );

    let mut bb_order: Vec<usize> = (0..prepared.billboards.len()).collect();
    bb_order.sort_by(|&a, &b| {
        let da = player_spawn.distance_squared(prepared.billboards[a].position);
        let db = player_spawn.distance_squared(prepared.billboards[b].position);
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut actor_order: Vec<usize> = (0..prepared.actors.len()).collect();
    actor_order.sort_by(|&a, &b| {
        let pa = &prepared.actors[a];
        let pb = &prepared.actors[b];
        let da = (pa.position[0] as f32 - player_spawn.x).powi(2)
            + (pa.position[1] as f32 + player_spawn.z).powi(2);
        let db = (pb.position[0] as f32 - player_spawn.x).powi(2)
            + (pb.position[1] as f32 + player_spawn.z).powi(2);
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut monster_order: Vec<usize> = (0..prepared.monsters.len()).collect();
    monster_order.sort_by(|&a, &b| {
        let pa = &prepared.monsters[a];
        let pb = &prepared.monsters[b];
        let da = (pa.position[0] as f32 - player_spawn.x).powi(2)
            + (pa.position[1] as f32 + player_spawn.z).powi(2);
        let db = (pb.position[0] as f32 - player_spawn.x).powi(2)
            + (pb.position[1] as f32 + player_spawn.z).powi(2);
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    });

    commands.insert_resource(PendingSpawns {
        billboard_order: bb_order,
        actor_order,
        monster_order,
        idx: 0,
        sprite_cache: prepared.sprite_cache.clone(),
        billboard_cache: prepared.billboard_cache.clone(),
        terrain_entity,
    });
}
/// Spawn a small fixed batch of entities per frame. No distance check — just sequential.
fn lazy_spawn(
    mut commands: Commands,
    pending: Option<ResMut<PendingSpawns>>,
    prepared: Option<Res<PreparedWorld>>,
    game_assets: Res<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (Some(mut pending), Some(prepared)) = (pending, prepared) else {
        return;
    };
    let terrain_entity = pending.terrain_entity;
    let npc_sprites = [("pfemst", "pfemwa"), ("pmanst", "pmanwk"), ("pmn2st", "pmn2wa")];
    let mut spawned = 0;
    let bb_len = pending.billboard_order.len();
    let actor_len = pending.actor_order.len();
    let monster_len = pending.monster_order.len();
    let mut bb_idx = pending.idx.min(bb_len);
    let mut actor_idx = pending.idx.saturating_sub(bb_len).min(actor_len);
    let mut monster_idx = pending.idx.saturating_sub(bb_len + actor_len).min(monster_len);

    // Billboards — decode sprite on first encounter, cache material
    while bb_idx < bb_len && spawned < SPAWN_BATCH_SIZE {
        let idx = pending.billboard_order[bb_idx];
        bb_idx += 1;
        pending.idx += 1;
        let bb = &prepared.billboards[idx];
        let key = &bb.declist_name;
        let (mat, quad, h) = if let Some((m, q, h)) = pending.billboard_cache.get(key) {
            (m.clone(), q.clone(), *h)
        } else {
            // First time — decode from LOD
            let bb_mgr_result = lod::billboard::BillboardManager::new(game_assets.lod_manager());
            let bb_mgr = match &bb_mgr_result {
                Ok(m) => m,
                Err(_) => continue,
            };
            let sprite = match bb_mgr.get(game_assets.lod_manager(), key, bb.declist_id) {
                Some(s) => s,
                None => continue,
            };
            let (w, h) = sprite.dimensions();
            let bevy_img = bevy::image::Image::from_dynamic(
                sprite.image, true, bevy::asset::RenderAssetUsages::RENDER_WORLD);
            let tex = images.add(bevy_img);
            let m = materials.add(StandardMaterial {
                base_color_texture: Some(tex), alpha_mode: AlphaMode::Mask(0.5),
                cull_mode: None, double_sided: true, unlit: true, ..default()
            });
            let q = meshes.add(Rectangle::new(w, h));
            pending.billboard_cache.insert(key.clone(), (m.clone(), q.clone(), h));
            (m, q, h)
        };
        let pos = bb.position + Vec3::new(0.0, h / 2.0, 0.0);
        commands.entity(terrain_entity).with_child((
            Name::new("decoration"), Mesh3d(quad), MeshMaterial3d(mat),
            Transform::from_translation(pos),
            crate::game::entities::WorldEntity,
            crate::game::entities::EntityKind::Decoration,
            crate::game::entities::Billboard,
        ));
        spawned += 1;
    }

    // NPC actors
    while actor_idx < actor_len && spawned < SPAWN_BATCH_SIZE {
        let i = pending.actor_order[actor_idx];
        actor_idx += 1;
        pending.idx += 1;
        let a = &prepared.actors[i];
        if a.hp <= 0 || a.position[0].abs() > 20000 || a.position[1].abs() > 20000 { continue; }

        let (st, wa) = npc_sprites[i % npc_sprites.len()];
        let (standing, sw, sh) = sprites::load_sprite_frames_cached(
            st, game_assets.lod_manager(), &mut images, &mut materials, &mut Some(&mut pending.sprite_cache));
        if standing.is_empty() { continue; }
        let (walking, _, _) = sprites::load_sprite_frames_cached(
            wa, game_assets.lod_manager(), &mut images, &mut materials, &mut Some(&mut pending.sprite_cache));
        let mut states = vec![standing];
        if !walking.is_empty() { states.push(walking); }
        let initial_mat = states[0][0][0].clone();
        let quad = meshes.add(Rectangle::new(sw, sh));
        let wx = a.position[0] as f32;
        let wz = -a.position[1] as f32;
        let gy = crate::game::collision::probe_ground_height(&prepared.map.height_map[..], None, wx, wz);
        let pos = Vec3::new(wx, gy + sh / 2.0, wz);

        commands.entity(terrain_entity).with_child((
            Name::new(format!("npc:{}", a.name)), Mesh3d(quad), MeshMaterial3d(initial_mat),
            Transform::from_translation(pos),
            crate::game::entities::WorldEntity, crate::game::entities::EntityKind::Npc,
            crate::game::entities::AnimationState::Idle,
            sprites::SpriteSheet { state_dimensions: states.iter().map(|_| (0.0, 0.0)).collect(), states, current_frame: 0, current_state: 0, frame_timer: 0.0, frame_duration: 0.15 },
            actor::Actor {
                name: a.name.clone(), hp: a.hp, max_hp: a.hp, move_speed: a.move_speed as f32,
                initial_position: pos, guarding_position: pos, tether_distance: a.tether_distance as f32,
                wander_timer: 0.0, wander_target: pos, facing_yaw: 0.0, hostile: false,
            },
        ));
        spawned += 1;
    }

    // Monsters
    while monster_idx < monster_len && spawned < SPAWN_BATCH_SIZE {
        let m = &prepared.monsters[pending.monster_order[monster_idx]];
        monster_idx += 1;
        pending.idx += 1;
        let fallbacks = [
            (m.standing_sprite.as_str(), m.walking_sprite.as_str()),
            ("pfemst", "pfemwa"), ("pmanst", "pmanwk"),
        ];
        let mut standing = Vec::new(); let mut walking = Vec::new();
        let mut sw = 0.0f32; let mut sh = 0.0f32;
        for (st, wa) in &fallbacks {
            let (sf, w, h) = sprites::load_sprite_frames_cached(
                st, game_assets.lod_manager(), &mut images, &mut materials, &mut Some(&mut pending.sprite_cache));
            if !sf.is_empty() && w > 0.0 {
                standing = sf; sw = w; sh = h;
                let (wf, _, _) = sprites::load_sprite_frames_cached(
                    wa, game_assets.lod_manager(), &mut images, &mut materials, &mut Some(&mut pending.sprite_cache));
                walking = wf; break;
            }
        }
        if standing.is_empty() { continue; }
        let mut states = vec![standing];
        if !walking.is_empty() { states.push(walking); }
        let initial_mat = states[0][0][0].clone();
        let quad = meshes.add(Rectangle::new(sw, sh));
        let wx = m.position[0] as f32;
        let wz = -m.position[1] as f32;
        let gy = crate::game::collision::probe_ground_height(&prepared.map.height_map[..], None, wx, wz);
        let pos = Vec3::new(wx, gy + sh / 2.0, wz);

        commands.entity(terrain_entity).with_child((
            Name::new("monster"), Mesh3d(quad), MeshMaterial3d(initial_mat),
            Transform::from_translation(pos),
            crate::game::entities::WorldEntity, crate::game::entities::EntityKind::Monster,
            crate::game::entities::AnimationState::Idle,
            sprites::SpriteSheet { state_dimensions: states.iter().map(|_| (0.0, 0.0)).collect(), states, current_frame: 0, current_state: 0, frame_timer: 0.0, frame_duration: 0.15 },
            actor::Actor {
                name: "Monster".into(), hp: 10, max_hp: 10, move_speed: m.move_speed as f32,
                initial_position: pos, guarding_position: pos, tether_distance: m.radius.max(200) as f32,
                wander_timer: 0.0, wander_target: pos, facing_yaw: 0.0, hostile: true,
            },
        ));
        spawned += 1;
    }

    if pending.idx >= bb_len + actor_len + monster_len {
        commands.remove_resource::<PendingSpawns>();
    }
}
