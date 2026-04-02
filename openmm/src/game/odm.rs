use bevy::prelude::*;
use bevy::render::render_resource::Face;

use crate::GameState;
use crate::game::InGame;
use crate::assets::GameAssets;
use crate::game::entities::{actor, sprites};
use crate::game::terrain_material::{TerrainMaterial, WaterExtension};

/// Spawn progress visible to the debug HUD.
#[derive(Resource, Default)]
pub struct SpawnProgress {
    pub total: usize,
    pub done: usize,
}

/// Pending entities sorted by distance from player, spawned gradually.
#[derive(Resource)]
struct PendingSpawns {
    billboard_order: Vec<usize>,
    actor_order: Vec<usize>,
    monster_order: Vec<usize>,
    idx: usize,
    frames_elapsed: u32,
    sprite_cache: sprites::SpriteCache,
    /// Cached billboard materials: key = sprite name, value = (material, mesh, height)
    billboard_cache: std::collections::HashMap<String, (Handle<StandardMaterial>, Handle<Mesh>, f32)>,
    /// Pre-resolved decoration entries for this map (directional detection, sprite names, dimensions).
    decorations: lod::game::decorations::Decorations,
    monsters: lod::game::monster::Monsters,
    /// Pre-resolved DDM actors (NPCs and monsters) for this map.
    actors: Option<lod::game::actors::Actors>,
    terrain_entity: Entity,
}
use crate::states::loading::PreparedWorld;

/// Grid coordinate for outdoor maps. Columns a-e, rows 1-3.
#[derive(Clone, Debug)]
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
/// On the first frame, spawn all entities with no budget limit.
/// The loading-to-game transition masks this single long frame.
const EAGER_SPAWN_FRAMES: u32 = 1;

pub struct OdmPlugin;

impl Plugin for OdmPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpawnProgress>()
            .add_systems(OnEnter(GameState::Game), spawn_world)
            .add_systems(
                Update,
                (lazy_spawn, check_map_boundary)
                    .run_if(in_state(GameState::Game))
                    .run_if(resource_equals(crate::game::hud::HudView::World)),
            );
    }
}

/// Half-size of the playable area in world units.
const PLAY_BOUNDARY: f32 = lod::odm::ODM_TILE_SCALE * lod::odm::ODM_PLAY_SIZE as f32 / 2.0;
/// Full playable area width (used to translate player position to new map).
pub const PLAY_WIDTH: f32 = lod::odm::ODM_TILE_SCALE * lod::odm::ODM_PLAY_SIZE as f32;

/// Detect when the player crosses the play area boundary and load the adjacent map.
fn check_map_boundary(
    mut commands: Commands,
    mut world_state: ResMut<crate::game::world_state::WorldState>,
    mut save_data: ResMut<crate::save::GameSave>,
    mut game_state: ResMut<NextState<GameState>>,
    player_query: Query<&Transform, With<crate::game::player::Player>>,
    load_request: Option<Res<crate::states::loading::LoadRequest>>,
) {
    // Don't trigger boundary crossing if a map transition is already queued
    if load_request.is_some() {
        debug!("check_map_boundary: skipped (LoadRequest exists)");
        return;
    }
    let Ok(transform) = player_query.single() else { return };
    let crate::game::map_name::MapName::Outdoor(ref odm) = world_state.map.name else {
        debug!("check_map_boundary: skipped (not outdoor map: {:?})", world_state.map.name);
        return;
    };
    let pos = transform.translation;
    let (yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);

    // Check which boundary was crossed (Bevy: +X=east, -X=west, -Z=north, +Z=south)
    let (new_odm, new_x, new_z) = if pos.x > PLAY_BOUNDARY {
        // East edge → load eastern map, player appears at western edge
        (odm.go_east(), pos.x - PLAY_WIDTH, pos.z)
    } else if pos.x < -PLAY_BOUNDARY {
        // West edge → load western map, player appears at eastern edge
        (odm.go_west(), pos.x + PLAY_WIDTH, pos.z)
    } else if pos.z < -PLAY_BOUNDARY {
        // North edge (Bevy -Z = MM6 +Y = north)
        (odm.go_north(), pos.x, pos.z + PLAY_WIDTH)
    } else if pos.z > PLAY_BOUNDARY {
        // South edge (Bevy +Z = MM6 -Y = south)
        (odm.go_south(), pos.x, pos.z - PLAY_WIDTH)
    } else {
        return; // Still inside playable area
    };

    let Some(new_odm) = new_odm else {
        return; // No adjacent map (edge of the world grid)
    };

    info!("Map transition: {} → {}", world_state.map.name, new_odm);

    // Update world state and save data for the new map
    world_state.map.name = crate::game::map_name::MapName::Outdoor(new_odm.clone());
    world_state.map.map_x = new_odm.x;
    world_state.map.map_y = new_odm.y;
    world_state.player.position = Vec3::new(new_x, pos.y, new_z);
    world_state.player.yaw = yaw;
    world_state.write_to_save(&mut save_data);

    commands.insert_resource(crate::states::loading::LoadRequest {
        map_name: world_state.map.name.clone(),
        spawn_position: None,
        spawn_yaw: None,
    });
    game_state.set(GameState::Loading);
}

fn spawn_world(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut terrain_materials: ResMut<Assets<TerrainMaterial>>,
    mut prepared: Option<ResMut<PreparedWorld>>,
    game_assets: Res<GameAssets>,
    save_data: Res<crate::save::GameSave>,
    cfg: Res<crate::config::GameConfig>,
    world_state: Res<crate::game::world_state::WorldState>,
    mut music_events: bevy::ecs::message::MessageWriter<crate::game::sound::music::PlayMusicEvent>,
) {
    let Some(prepared) = prepared.as_mut() else {
        // No outdoor PreparedWorld — this is an indoor map, skip outdoor spawning
        return;
    };

    // Load event data for this outdoor map (use current_map, not save_data which may be stale)
    let map_base = match &world_state.map.name {
        crate::game::map_name::MapName::Outdoor(odm) => format!("out{}{}", odm.x, odm.y),
        _ => return, // shouldn't happen — we checked PreparedWorld exists
    };
    crate::game::events::load_map_events(&mut commands, &game_assets, &map_base, false);

    let mut terrain_texture = prepared.terrain_texture.clone();
    let terrain_mesh = prepared.terrain_mesh.clone();

    // Cyan markers have been replaced with neutral color by extract_water_mask(),
    // so the atlas can safely use linear filtering without cyan bleed.
    terrain_texture.sampler = crate::assets::sampler_for_filtering(&cfg.terrain_filtering);
    let terrain_tex_handle = images.add(terrain_texture);

    // Water mask: R8 texture with nearest filtering for sharp water boundaries
    let water_mask_handle = if let Some(ref mask) = prepared.water_mask {
        let mut m = mask.clone();
        m.sampler = crate::assets::nearest_sampler();
        images.add(m)
    } else {
        images.add(Image::default())
    };

    // Water texture uses same filtering as terrain for visual consistency
    let water_sampler = crate::assets::sampler_for_filtering(&cfg.terrain_filtering);
    let water_tex_handle = if let Some(ref water_tex) = prepared.water_texture {
        let mut water = water_tex.clone();
        water.sampler = water_sampler;
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
            water_mask: water_mask_handle,
        },
    });

    let terrain_entity = commands
        .spawn((
            Name::new("odm"),
            Mesh3d(meshes.add(terrain_mesh)),
            MeshMaterial3d(terrain_mat),
            Transform::default(),
            Visibility::default(),
            InGame,
        ))
        .with_children(|parent| {
            let model_sampler = crate::assets::sampler_for_filtering(&cfg.models_filtering);
            // BSP models (buildings, structures)
            for model in &prepared.models {
                let is_building = !model.event_ids.is_empty();
                let mut model_entity = parent.spawn((
                    Name::new(format!("model_{}", model.name)),
                    Transform::default(),
                    Visibility::default(),
                ));

                if is_building {
                    model_entity.insert(
                        crate::game::interaction::make_building_info(&model.name, model.position, model.event_ids.clone()),
                    );
                }

                model_entity.with_children(|model_parent| {
                    for sub in &model.sub_meshes {
                        let mut mat = sub.material.clone();
                        if let Some(ref tex) = sub.texture {
                            let mut img = tex.clone();
                            img.sampler = model_sampler.clone();
                            let tex_handle = images.add(img);
                            mat.base_color_texture = Some(tex_handle);
                        }
                        model_parent.spawn((
                            Mesh3d(meshes.add(sub.mesh.clone())),
                            MeshMaterial3d(materials.add(mat)),
                        ));
                    }
                });
            }
        }).id();

    // Build outdoor clickable faces from BSP model faces with cog_trigger_id
    {
        let mut outdoor_clickable = Vec::new();
        for model in &prepared.map.bsp_models {
            for face in &model.faces {
                if face.cog_trigger_id == 0 || face.vertices_count < 3 || face.is_invisible() {
                    continue;
                }
                let vc = face.vertices_count as usize;
                let verts: Vec<Vec3> = (0..vc)
                    .filter_map(|i| {
                        let idx = face.vertices_ids[i] as usize;
                        model.vertices.get(idx).map(|v| Vec3::from(*v))
                    })
                    .collect();
                if verts.len() < 3 { continue; }
                let nx = face.plane.normal[0] as f32 / 65536.0;
                let ny = face.plane.normal[2] as f32 / 65536.0;
                let nz = -face.plane.normal[1] as f32 / 65536.0;
                let normal = Vec3::new(nx, ny, nz);
                let plane_dist = normal.dot(verts[0]);
                outdoor_clickable.push(crate::game::blv::ClickableFaceInfo {
                    face_index: 0,
                    event_id: face.cog_trigger_id,
                    normal,
                    plane_dist,
                    vertices: verts,
                });
            }
        }
        if !outdoor_clickable.is_empty() {
            commands.insert_resource(crate::game::blv::ClickableFaces {
                faces: outdoor_clickable,
            });
        }
    }

    let decorations = prepared.decorations.clone();

    // Reuse Actors and Monsters resolved during preloading — avoids duplicate LOD parsing.
    let monsters = prepared.resolved_monsters.take()
        .unwrap_or_else(lod::game::monster::Monsters::default_empty);
    let actors = prepared.resolved_actors.take();

    // Sort spawn order by distance from player (closest first)
    let player_spawn = Vec3::new(
        save_data.player.position[0],
        save_data.player.position[1],
        save_data.player.position[2],
    );

    let bb_order = sort_by_distance_mm6(decorations.entries(), player_spawn,
        |d| d.position[0] as f32, |d| d.position[1] as f32);

    let actor_order = if let Some(ref a) = actors {
        sort_by_distance_mm6(a.get_actors(), player_spawn,
            |actor| actor.position[0] as f32,
            |actor| actor.position[1] as f32)
    } else {
        Vec::new()
    };

    let monster_order = sort_by_distance_mm6(monsters.entries(), player_spawn,
        |m| m.spawn_position[0] as f32, |m| m.spawn_position[1] as f32);

    let total = bb_order.len() + actor_order.len() + monster_order.len();
    // Play map music
    music_events.write(crate::game::sound::music::PlayMusicEvent {
        track: prepared.music_track,
        volume: cfg.music_volume,
    });

    commands.insert_resource(SpawnProgress { total, done: 0 });
    commands.insert_resource(PendingSpawns {
        billboard_order: bb_order,
        actor_order,
        monster_order,
        idx: 0,
        frames_elapsed: 0,
        sprite_cache: prepared.sprite_cache.clone(),
        billboard_cache: prepared.billboard_cache.clone(),
        decorations,
        monsters,
        actors,
        terrain_entity,
    });
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
    mut progress: ResMut<SpawnProgress>,
    mut sound_events: bevy::ecs::message::MessageWriter<crate::game::sound::effects::PlaySoundEvent>,
    mut map_events: Option<ResMut<crate::game::events::MapEvents>>,
) {
    let (Some(mut pending), Some(prepared)) = (pending, prepared) else {
        return;
    };
    let p = &mut *pending;
    let terrain_entity = p.terrain_entity;
    let mut spawned = 0;
    let start = std::time::Instant::now();

    // On the first frame, spawn all entities with no budget — the loading-to-game
    // transition masks this single long frame. After that, use the normal budget.
    let eager = p.frames_elapsed < EAGER_SPAWN_FRAMES;
    let time_budget = if eager { f32::MAX } else { SPAWN_TIME_BUDGET_MS };
    let batch_max = if eager { usize::MAX } else { SPAWN_BATCH_MAX };
    p.frames_elapsed += 1;

    let bb_len = p.billboard_order.len();
    let actor_len = p.actor_order.len();
    let monster_len = p.monster_order.len();
    let mut bb_idx = p.idx.min(bb_len);
    let mut actor_idx = p.idx.saturating_sub(bb_len).min(actor_len);
    let mut monster_idx = p.idx.saturating_sub(bb_len + actor_len).min(monster_len);

    // Billboards — directional decorations (e.g. ships) get SpriteSheet + FacingYaw
    // so they show the correct side based on camera angle.
    let bb_mgr = game_assets.billboard_manager();
    while bb_idx < bb_len && spawned < batch_max && start.elapsed().as_secs_f32() * 1000.0 < time_budget {
        let dec_idx = p.billboard_order[bb_idx];
        bb_idx += 1;
        p.idx += 1;
        let dec = &p.decorations.entries()[dec_idx];
        let key = &dec.sprite_name;
        let dec_pos = Vec3::from(lod::odm::mm6_to_bevy(dec.position[0], dec.position[1], dec.position[2]));

        if dec.is_directional {
            let (dirs, px_w, px_h) = sprites::load_decoration_directions(
                &dec.sprite_name, game_assets.lod_manager(),
                &mut images, &mut materials, &mut Some(&mut p.sprite_cache));
            if px_w > 0.0 {
                // Apply DSFT scale via sprite group name lookup
                let dsft_scale = bb_mgr.dsft_scale_for_group(&dec.sprite_name);
                let sw = px_w * dsft_scale;
                let sh = px_h * dsft_scale;
                let initial_mat = dirs[0].clone();
                let quad = meshes.add(Rectangle::new(sw, sh));
                let pos = dec_pos + Vec3::new(0.0, sh / 2.0, 0.0);
                // Single animation frame with 5 directional views
                let states = vec![vec![dirs]];
                let child_id = commands.spawn((
                    Name::new(format!("decoration:{}", key)),
                    Mesh3d(quad), MeshMaterial3d(initial_mat),
                    Transform::from_translation(pos),
                    crate::game::entities::WorldEntity,
                    crate::game::entities::EntityKind::Decoration,
                    crate::game::entities::Billboard,
                    crate::game::entities::AnimationState::Idle,
                    sprites::SpriteSheet::new(states, vec![(sw, sh)]),
                    crate::game::entities::FacingYaw(dec.facing_yaw),
                )).id();
                commands.entity(terrain_entity).add_child(child_id);
                if dec.event_id > 0 {
                    commands.entity(child_id).insert(crate::game::interaction::DecorationInfo {
                        event_id: dec.event_id as u16,
                        position: pos,
                        billboard_index: dec.billboard_index,
                    });
                }
                spawned += 1;
            } else {
                continue;
            }
        } else {
            let (mat, quad, h) = if let Some((m, q, h)) = p.billboard_cache.get(key) {
                (m.clone(), q.clone(), *h)
            } else {
                let sprite = match bb_mgr.get(game_assets.lod_manager(), key, dec.declist_id) {
                    Some(s) => s,
                    None => {
                        warn!("Billboard '{}' sprite not found, skipping", key);
                        continue;
                    }
                };
                let (w, h) = sprite.dimensions();
                let bevy_img = crate::assets::dynamic_to_bevy_image(sprite.image);
                let tex = images.add(bevy_img);
                let m = materials.add(StandardMaterial {
                    unlit: true,
                    base_color_texture: Some(tex), alpha_mode: AlphaMode::Mask(0.5),
                    cull_mode: None, double_sided: true,
                    perceptual_roughness: 1.0, reflectance: 0.0,
                    ..default()
                });
                let q = meshes.add(Rectangle::new(w, h));
                p.billboard_cache.insert(key.clone(), (m.clone(), q.clone(), h));
                (m, q, h)
            };
            let pos = dec_pos + Vec3::new(0.0, h / 2.0, 0.0);
            let child_id = commands.spawn((
                Name::new("decoration"), Mesh3d(quad), MeshMaterial3d(mat),
                Transform::from_translation(pos),
                crate::game::entities::WorldEntity,
                crate::game::entities::EntityKind::Decoration,
                crate::game::entities::Billboard,
            )).id();
            commands.entity(terrain_entity).add_child(child_id);
            if dec.event_id > 0 {
                commands.entity(child_id).insert(crate::game::interaction::DecorationInfo {
                    event_id: dec.event_id as u16,
                    position: pos,
                    billboard_index: dec.billboard_index,
                });
            }
            spawned += 1;
        }

        if dec.sound_id > 0 {
            sound_events.write(crate::game::sound::effects::PlaySoundEvent {
                sound_id: dec.sound_id as u32,
                position: dec_pos,
            });
        }
    }

    // NPC actors
    while actor_idx < actor_len && spawned < batch_max && start.elapsed().as_secs_f32() * 1000.0 < time_budget {
        let i = p.actor_order[actor_idx];
        actor_idx += 1;
        p.idx += 1;

        let actor = match p.actors.as_ref().and_then(|a| a.get_actors().get(i)) {
            Some(a) => a,
            None => continue,
        };

        let variant = actor.variant;

        let (s2, w2, h2) = sprites::load_entity_sprites(
            &actor.standing_sprite, &actor.walking_sprite, game_assets.lod_manager(),
            &mut images, &mut materials, &mut Some(&mut p.sprite_cache), variant, actor.palette_id);
        if s2.is_empty() || s2[0].is_empty() {
            error!("NPC '{}' monlist_id={} sprite '{}'/'{}'  failed to load",
                actor.name, actor.monlist_id, actor.standing_sprite, actor.walking_sprite);
            continue;
        }
        // Apply DSFT scale (same as decorations — sprite group name lookup)
        let dsft_scale = bb_mgr.dsft_scale_for_group(&actor.standing_sprite);
        let states = s2;
        let sw = w2 * dsft_scale;
        let sh = h2 * dsft_scale;
        let initial_mat = states[0][0][0].clone();
        let quad = meshes.add(Rectangle::new(sw, sh));
        let wx = actor.position[0] as f32;
        let wz = -actor.position[1] as f32;
        let terrain_y = crate::game::collision::probe_ground_height(&prepared.map.height_map[..], None, wx, wz);
        // Use DDM Z position when above terrain (e.g. NPC on a balcony/building).
        // Both terrain_y and DDM position[2] are in raw game units — no extra scaling needed.
        let ddm_y = actor.position[2] as f32;
        let gy = terrain_y.max(ddm_y);
        let pos = Vec3::new(wx, gy + sh / 2.0, wz);

        // Identity assignment: Actor already has name/portrait for named NPCs.
        // For peasants, assign identity from npcdata.txt via map_events.
        let (display_name, effective_npc_id) = if actor.is_peasant {
            let generated_id = 5000 + i as i32;
            // Pick a complete identity (name + portrait) from npcdata.txt peasant entries,
            // split by sex. Falls back to npcnames.txt name + generic portrait if unavailable.
            let (name, portrait) = map_events.as_ref()
                .and_then(|me| me.npc_table.as_ref())
                .and_then(|t| t.peasant_identity(actor.is_female, i))
                .map(|(n, p)| (n.to_string(), p))
                .unwrap_or_else(|| {
                    let name = map_events.as_ref()
                        .and_then(|me| me.name_pool.as_ref())
                        .map(|pool| pool.name_for(actor.is_female, i).to_string())
                        .unwrap_or_else(|| actor.name.clone());
                    (name, 1)
                });
            if let Some(ref mut me) = map_events {
                me.generated_npcs.insert(generated_id, lod::game::npc::GeneratedNpc {
                    name: name.clone(), portrait,
                });
            }
            (name, generated_id)
        } else {
            // Named NPC: name already resolved in Actor
            (actor.name.clone(), actor.npc_id() as i32)
        };

        commands.entity(terrain_entity).with_child((
            Name::new(format!("npc:{}", actor.name)), Mesh3d(quad), MeshMaterial3d(initial_mat),
            Transform::from_translation(pos),
            crate::game::entities::WorldEntity, crate::game::entities::EntityKind::Npc,
            crate::game::entities::AnimationState::Idle,
            sprites::SpriteSheet::new(states, vec![(sw, sh)]),
            actor::Actor {
                name: actor.name.clone(), hp: actor.hp, max_hp: actor.hp,
                move_speed: actor.move_speed as f32,
                initial_position: pos, guarding_position: pos,
                tether_distance: actor.tether_distance as f32,
                wander_timer: (pos.x * 0.011 + pos.z * 0.017).abs().fract() * 4.0,
                wander_target: pos, facing_yaw: 0.0, hostile: false,
            },
            crate::game::interaction::NpcInteractable {
                name: display_name,
                position: pos,
                npc_id: effective_npc_id as i16,
            },
        ));
        spawned += 1;
    }

    // Monsters
    while monster_idx < monster_len && spawned < batch_max && start.elapsed().as_secs_f32() * 1000.0 < time_budget {
        let m = &p.monsters.entries()[p.monster_order[monster_idx]];
        monster_idx += 1;
        p.idx += 1;
        let (states, raw_w, raw_h) = sprites::load_entity_sprites(
            &m.standing_sprite, &m.walking_sprite, game_assets.lod_manager(),
            &mut images, &mut materials, &mut Some(&mut p.sprite_cache), m.variant, m.palette_id);
        if states.is_empty() || states[0].is_empty() {
            error!("Monster sprite '{}'/'{}'  failed to load — skipping", m.standing_sprite, m.walking_sprite);
            continue;
        }
        // Apply DSFT scale (same as decorations — sprite group name lookup)
        let dsft_scale = bb_mgr.dsft_scale_for_group(&m.standing_sprite);
        let sw = raw_w * dsft_scale;
        let sh = raw_h * dsft_scale;
        let initial_mat = states[0][0][0].clone();
        let quad = meshes.add(Rectangle::new(sw, sh));
        // Compute spread position (was done inside resolve_monsters, now done here)
        let angle = m.group_index as f32 * 2.094;
        let spread = m.spawn_radius as f32 * 0.5;
        let wx = m.spawn_position[0] as f32 + angle.cos() * spread * m.group_index as f32;
        let wz = -(m.spawn_position[1] as f32 + angle.sin() * spread * m.group_index as f32);
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
                wander_timer: (pos.x * 0.011 + pos.z * 0.017).abs().fract() * 4.0, wander_target: pos, facing_yaw: 0.0, hostile: true,
            },
        ));
        spawned += 1;
    }

    progress.done = p.idx;

    if p.idx >= bb_len + actor_len + monster_len {
        commands.remove_resource::<PendingSpawns>();
    }
}
