use bevy::prelude::*;
use bevy::render::render_resource::Face;

use crate::GameState;
use crate::game::InGame;
use crate::assets::GameAssets;
use crate::game::entities::{actor, sprites};
use crate::game::terrain_material::{TerrainMaterial, WaterExtension};

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
    resolved_monsters: Vec<crate::states::loading::PreparedMonster>,
    /// NPC sprite lookup: monster_id → (standing_root, walking_root)
    npc_sprite_table: std::collections::HashMap<u8, (String, String)>,
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

/// Max time budget per frame for entity spawning (milliseconds).
/// Keeps frame time from ballooning when spawning many entities.
const SPAWN_TIME_BUDGET_MS: f32 = 4.0;
/// Hard cap on entities per frame even if time budget allows.
const SPAWN_BATCH_MAX: usize = 12;

pub struct OdmPlugin;

impl Plugin for OdmPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Game), spawn_world)
            .add_systems(
                Update,
                (lazy_spawn, check_map_boundary)
                    .run_if(in_state(GameState::Game)),
            );
    }
}

/// Half-size of the playable area in world units.
const PLAY_BOUNDARY: f32 = lod::odm::ODM_TILE_SCALE * lod::odm::ODM_PLAY_SIZE as f32 / 2.0;
/// Full playable area width (used to translate player position to new map).
const PLAY_WIDTH: f32 = lod::odm::ODM_TILE_SCALE * lod::odm::ODM_PLAY_SIZE as f32;

/// Detect when the player crosses the play area boundary and load the adjacent map.
fn check_map_boundary(
    mut commands: Commands,
    mut current_map: ResMut<crate::game::debug::CurrentMapName>,
    mut save_data: ResMut<crate::save::GameSave>,
    mut game_state: ResMut<NextState<GameState>>,
    player_query: Query<&Transform, With<crate::game::player::Player>>,
) {
    let Ok(transform) = player_query.single() else { return };
    let pos = transform.translation;
    let (yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);

    // Check which boundary was crossed (Bevy: +X=east, -X=west, -Z=north, +Z=south)
    let (new_map, new_x, new_z) = if pos.x > PLAY_BOUNDARY {
        // East edge → load eastern map, player appears at western edge
        (current_map.0.go_east(), pos.x - PLAY_WIDTH, pos.z)
    } else if pos.x < -PLAY_BOUNDARY {
        // West edge → load western map, player appears at eastern edge
        (current_map.0.go_west(), pos.x + PLAY_WIDTH, pos.z)
    } else if pos.z < -PLAY_BOUNDARY {
        // North edge (Bevy -Z = MM6 +Y = north)
        (current_map.0.go_north(), pos.x, pos.z + PLAY_WIDTH)
    } else if pos.z > PLAY_BOUNDARY {
        // South edge (Bevy +Z = MM6 -Y = south)
        (current_map.0.go_south(), pos.x, pos.z - PLAY_WIDTH)
    } else {
        return; // Still inside playable area
    };

    let Some(new_map) = new_map else {
        return; // No adjacent map (edge of the world grid)
    };

    info!("Map transition: {} → {}", current_map.0, new_map);

    // Save player position translated to the new map's coordinate space
    save_data.player.position = [new_x, pos.y, new_z];
    save_data.player.yaw = yaw;
    save_data.map.map_x = new_map.x;
    save_data.map.map_y = new_map.y;

    commands.insert_resource(crate::states::loading::LoadRequest {
        map_name: new_map.clone(),
    });
    current_map.0 = new_map;
    game_state.set(GameState::Loading);
}

fn spawn_world(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut terrain_materials: ResMut<Assets<TerrainMaterial>>,
    prepared: Option<Res<PreparedWorld>>,
    game_assets: Res<GameAssets>,
    save_data: Res<crate::save::GameSave>,
) {
    let Some(prepared) = prepared else {
        error!("No PreparedWorld available when entering Game state");
        return;
    };

    let mut terrain_texture = prepared.terrain_texture.clone();
    let terrain_mesh = prepared.terrain_mesh.clone();

    // Nearest-neighbor filtering on terrain atlas to prevent cyan water markers
    // from bleeding into neighboring terrain tiles via bilinear interpolation.
    terrain_texture.sampler = bevy::image::ImageSampler::Descriptor(
        bevy::image::ImageSamplerDescriptor {
            min_filter: bevy::image::ImageFilterMode::Nearest,
            mag_filter: bevy::image::ImageFilterMode::Nearest,
            mipmap_filter: bevy::image::ImageFilterMode::Nearest,
            ..default()
        },
    );
    let terrain_tex_handle = images.add(terrain_texture);

    // Load water texture (or create a placeholder if missing)
    let water_tex_handle = if let Some(ref water_tex) = prepared.water_texture {
        let mut water = water_tex.clone();
        water.sampler = bevy::image::ImageSampler::Descriptor(
            bevy::image::ImageSamplerDescriptor {
                address_mode_u: bevy::image::ImageAddressMode::Repeat,
                address_mode_v: bevy::image::ImageAddressMode::Repeat,
                min_filter: bevy::image::ImageFilterMode::Linear,
                mag_filter: bevy::image::ImageFilterMode::Linear,
                ..default()
            },
        );
        images.add(water)
    } else {
        images.add(Image::default())
    };

    let terrain_mat = terrain_materials.add(TerrainMaterial {
        base: StandardMaterial {
            base_color_texture: Some(terrain_tex_handle),
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            metallic: 0.0,
            cull_mode: Some(Face::Back),
            ..default()
        },
        extension: WaterExtension {
            water_texture: water_tex_handle,
        },
    });

    let terrain_entity = commands
        .spawn((
            Name::new("odm"),
            Mesh3d(meshes.add(terrain_mesh)),
            MeshMaterial3d(terrain_mat),
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

    // Resolve monsters from spawn points (cheap — no sprite loading)
    let resolved_monsters = resolve_monsters(&prepared, &game_assets, &save_data);

    // Build NPC sprite lookup from monlist — each DDM actor has a monster_id
    // that maps to a monlist entry with specific sprite names.
    let npc_sprite_table = build_npc_sprite_table(&game_assets);

    // Sort spawn order by distance from player (closest first)
    let player_spawn = Vec3::new(
        save_data.player.position[0],
        save_data.player.position[1],
        save_data.player.position[2],
    );

    let bb_order = sort_by_distance_vec3(&prepared.billboards, player_spawn, |bb| bb.position);

    let actor_order = sort_by_distance_mm6(&prepared.actors, player_spawn,
        |a| a.position[0] as f32, |a| a.position[1] as f32);

    let monster_order = sort_by_distance_mm6(&resolved_monsters, player_spawn,
        |m| m.position[0] as f32, |m| m.position[1] as f32);

    commands.insert_resource(PendingSpawns {
        billboard_order: bb_order,
        actor_order,
        monster_order,
        idx: 0,
        sprite_cache: prepared.sprite_cache.clone(),
        billboard_cache: prepared.billboard_cache.clone(),
        resolved_monsters,
        npc_sprite_table,
        terrain_entity,
    });
}

/// Resolve monsters from spawn points using mapstats + monlist.
fn resolve_monsters(
    prepared: &PreparedWorld,
    game_assets: &GameAssets,
    save_data: &crate::save::GameSave,
) -> Vec<crate::states::loading::PreparedMonster> {
    let mut monsters = Vec::new();
    let Ok(mapstats) = lod::mapstats::MapStats::new(game_assets.lod_manager()) else { return monsters };
    let Ok(monlist) = lod::monlist::MonsterList::new(game_assets.lod_manager()) else { return monsters };

    let map_config = mapstats.get(&format!("out{}{}.odm", save_data.map.map_x, save_data.map.map_y));
    let Some(cfg) = map_config else { return monsters };

    for sp in &prepared.map.spawn_points {
        // Seed for random A/B/C variant based on position (deterministic)
        let seed = (sp.position[0].unsigned_abs() + sp.position[1].unsigned_abs()) as u32;
        let Some((mon_name, dif)) = cfg.monster_for_index(sp.monster_index, seed) else { continue };
        let Some(desc) = monlist.find_with_sprite(mon_name, dif, game_assets.lod_manager()) else { continue };

        let group_size = 3 + ((sp.position[0].unsigned_abs() + sp.position[1].unsigned_abs()) % 3) as i32;
        for g in 0..group_size {
            let angle = g as f32 * 2.094;
            let spread = sp.radius.max(200) as f32 * 0.5;
            monsters.push(crate::states::loading::PreparedMonster {
                position: [
                    sp.position[0] + (angle.cos() * spread * g as f32) as i32,
                    sp.position[1] + (angle.sin() * spread * g as f32) as i32,
                    sp.position[2],
                ],
                radius: sp.radius.max(300),
                standing_sprite: desc.sprite_names[0].clone(),
                walking_sprite: desc.sprite_names[1].clone(),
                height: desc.height,
                move_speed: desc.move_speed,
                hostile: true,
                variant: dif,
            });
        }
    }
    monsters
}

/// Build a lookup table: monster_id → (standing_sprite, walking_sprite) from monlist.
/// Only includes entries whose sprites actually exist in the LOD.
fn build_npc_sprite_table(game_assets: &GameAssets) -> std::collections::HashMap<u8, (String, String)> {
    let mut table = std::collections::HashMap::new();
    let Ok(monlist) = lod::monlist::MonsterList::new(game_assets.lod_manager()) else {
        return table;
    };
    let lod = game_assets.lod_manager();
    for (i, desc) in monlist.monsters.iter().enumerate() {
        if i > 255 { break; }
        let st = &desc.sprite_names[0];
        if st.is_empty() { continue; }
        // Check if the standing sprite actually exists in the LOD
        let root = st.trim_end_matches(|c: char| c.is_ascii_digit());
        let mut found = false;
        let mut try_root = root;
        while try_root.len() >= 3 {
            let test = format!("{}a0", try_root);
            if lod.try_get_bytes(&format!("sprites/{}", test)).is_ok() {
                found = true;
                break;
            }
            try_root = &try_root[..try_root.len() - 1];
        }
        if found {
            table.insert(i as u8, (st.clone(), desc.sprite_names[1].clone()));
        }
    }
    table
}

/// Sort indices by distance from player using Vec3 positions.
fn sort_by_distance_vec3<T>(items: &[T], player: Vec3, pos: impl Fn(&T) -> Vec3) -> Vec<usize> {
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by(|&a, &b| {
        let da = player.distance_squared(pos(&items[a]));
        let db = player.distance_squared(pos(&items[b]));
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    });
    order
}

/// Sort indices by distance from player using MM6 coords (works with i16 or i32).
fn sort_by_distance_mm6<T>(items: &[T], player: Vec3, pos_x: impl Fn(&T) -> f32, pos_y: impl Fn(&T) -> f32) -> Vec<usize> {
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by(|&a, &b| {
        let da = (pos_x(&items[a]) - player.x).powi(2) + (pos_y(&items[a]) + player.z).powi(2);
        let db = (pos_x(&items[b]) - player.x).powi(2) + (pos_y(&items[b]) + player.z).powi(2);
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    });
    order
}

/// Spawn a small batch of entities per frame from the pending queue.
/// Uses a time budget to avoid spending too long per frame.
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
    let p = &mut *pending;
    let terrain_entity = p.terrain_entity;
    let npc_fallback = [("pfemst", "pfemwa"), ("pmanst", "pmanwa"), ("pmn2st", "pmn2wa")];
    let mut spawned = 0;
    let start = std::time::Instant::now();
    let bb_len = p.billboard_order.len();
    let actor_len = p.actor_order.len();
    let monster_len = p.resolved_monsters.len();
    let mut bb_idx = p.idx.min(bb_len);
    let mut actor_idx = p.idx.saturating_sub(bb_len).min(actor_len);
    let mut monster_idx = p.idx.saturating_sub(bb_len + actor_len).min(monster_len);

    // Billboards
    while bb_idx < bb_len && spawned < SPAWN_BATCH_MAX && start.elapsed().as_secs_f32() * 1000.0 < SPAWN_TIME_BUDGET_MS {
        let idx = p.billboard_order[bb_idx];
        bb_idx += 1;
        p.idx += 1;
        let bb = &prepared.billboards[idx];
        let key = &bb.declist_name;
        let (mat, quad, h) = if let Some((m, q, h)) = p.billboard_cache.get(key) {
            (m.clone(), q.clone(), *h)
        } else {
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
            p.billboard_cache.insert(key.clone(), (m.clone(), q.clone(), h));
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
    while actor_idx < actor_len && spawned < SPAWN_BATCH_MAX && start.elapsed().as_secs_f32() * 1000.0 < SPAWN_TIME_BUDGET_MS {
        let i = p.actor_order[actor_idx];
        actor_idx += 1;
        p.idx += 1;
        let a = &prepared.actors[i];
        if a.hp <= 0 || a.position[0].abs() > 20000 || a.position[1].abs() > 20000 { continue; }

        // Look up sprites from monlist via monster_id. Try loading, fall back to peasants.
        let mut states = Vec::new();
        let mut sw = 0.0f32;
        let mut sh = 0.0f32;

        if let Some((s, w)) = p.npc_sprite_table.get(&a.monster_id) {
            let (s2, w2, h2) = sprites::load_entity_sprites(
                s, w, game_assets.lod_manager(),
                &mut images, &mut materials, &mut Some(&mut p.sprite_cache), 0.0);
            if !s2.is_empty() && !s2[0].is_empty() {
                states = s2; sw = w2; sh = h2;
            }
        }
        if states.is_empty() {
            let (s, w) = npc_fallback[i % npc_fallback.len()];
            let (fb_states, fb_w, fb_h) = sprites::load_entity_sprites(
                s, w, game_assets.lod_manager(),
                &mut images, &mut materials, &mut Some(&mut p.sprite_cache), 0.0);
            states = fb_states; sw = fb_w; sh = fb_h;
            if states.is_empty() || states[0].is_empty() { continue; }
        }
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
            sprites::SpriteSheet::new(states, vec![(sw, sh)]),
            actor::Actor {
                name: a.name.clone(), hp: a.hp, max_hp: a.hp, move_speed: a.move_speed as f32,
                initial_position: pos, guarding_position: pos, tether_distance: a.tether_distance as f32,
                wander_timer: 0.0, wander_target: pos, facing_yaw: 0.0, hostile: false,
            },
        ));
        spawned += 1;
    }

    // Monsters
    while monster_idx < monster_len && spawned < SPAWN_BATCH_MAX && start.elapsed().as_secs_f32() * 1000.0 < SPAWN_TIME_BUDGET_MS {
        let m = &p.resolved_monsters[p.monster_order[monster_idx]];
        monster_idx += 1;
        p.idx += 1;
        // Hue shift for difficulty variants: A=0°, B=120° (blue), C=240° (red/green)
        let hue = match m.variant {
            2 => 120.0,
            3 => 240.0,
            _ => 0.0,
        };
        let sprite_pairs = [
            (m.standing_sprite.as_str(), m.walking_sprite.as_str()),
            ("pfemst", "pfemwa"), ("pmanst", "pmanwa"),
        ];
        let mut states = Vec::new();
        let mut sw = 0.0f32; let mut sh = 0.0f32;
        for (st, wa) in &sprite_pairs {
            let (s, w, h) = sprites::load_entity_sprites(
                st, wa, game_assets.lod_manager(),
                &mut images, &mut materials, &mut Some(&mut p.sprite_cache), hue);
            if !s.is_empty() && !s[0].is_empty() {
                states = s; sw = w; sh = h; break;
            }
        }
        if states.is_empty() { continue; }
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
            sprites::SpriteSheet::new(states, vec![(sw, sh)]),
            actor::Actor {
                name: "Monster".into(), hp: 10, max_hp: 10, move_speed: m.move_speed as f32,
                initial_position: pos, guarding_position: pos, tether_distance: m.radius.max(200) as f32,
                wander_timer: 0.0, wander_target: pos, facing_yaw: 0.0, hostile: true,
            },
        ));
        spawned += 1;
    }

    if p.idx >= bb_len + actor_len + monster_len {
        commands.remove_resource::<PendingSpawns>();
    }
}
