use bevy::ecs::message::MessageReader;
use bevy::prelude::*;
use bevy::render::render_resource::Face;

use crate::GameState;
use crate::assets::GameAssets;
use crate::game::InGame;
use crate::game::entities::{actor, sprites};
use crate::game::terrain_material::{TerrainMaterial, WaterExtension};

/// Marker on each outdoor BSP model sub-mesh entity — tracks which model and faces it represents.
#[derive(Component)]
pub struct BspSubMesh {
    /// Index of the BSP model this sub-mesh belongs to (index into `PreparedWorld::models`).
    pub model_index: u32,
    /// Face indices (into the BSPModel::faces array) that contributed to this sub-mesh.
    pub face_indices: Vec<u32>,
    /// Current texture name on this sub-mesh.
    pub texture_name: String,
}

/// Message to swap the texture on an outdoor BSP model face at runtime.
#[derive(Message)]
pub struct ApplyTextureOutdoors {
    pub model: u32,
    pub facet: u32,
    pub texture_name: String,
}

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
    idx: usize,
    frames_elapsed: u32,
    sprite_cache: sprites::SpriteCache,
    /// Cached billboard materials: key = sprite name, value = (material, mesh, width, height, mask)
    dec_sprite_cache: std::collections::HashMap<
        String,
        (
            Handle<StandardMaterial>,
            Handle<Mesh>,
            f32,
            f32,
            std::sync::Arc<crate::game::entities::sprites::AlphaMask>,
        ),
    >,
    /// Pre-resolved decoration entries for this map (directional detection, sprite names, dimensions).
    decorations: lod::game::decorations::Decorations,
    /// Pre-resolved DDM actors (NPCs only for outdoor maps) for this map.
    actors: Option<lod::game::actors::Actors>,
    /// ODM spawn-point monsters (outdoor only). Each entry is one group member.
    monsters: Option<lod::game::monster::Monsters>,
    monster_order: Vec<usize>,
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
            .add_message::<ApplyTextureOutdoors>()
            .add_systems(OnEnter(GameState::Game), spawn_world)
            .add_systems(
                Update,
                (lazy_spawn, check_map_boundary, apply_texture_outdoors)
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
        debug!(
            "check_map_boundary: skipped (not outdoor map: {:?})",
            world_state.map.name
        );
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
    save_data: Res<crate::save::GameSave>,
    cfg: Res<crate::config::GameConfig>,
    mut music_events: bevy::ecs::message::MessageWriter<crate::game::sound::music::PlayMusicEvent>,
) {
    let Some(prepared) = prepared.as_mut() else {
        // No outdoor PreparedWorld — this is an indoor map, skip outdoor spawning
        return;
    };

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
            for (model_index, model) in prepared.models.iter().enumerate() {
                let mut model_entity = parent.spawn((
                    Name::new(format!("model_{}", model.name)),
                    Transform::default(),
                    Visibility::default(),
                ));

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
                            BspSubMesh {
                                model_index: model_index as u32,
                                face_indices: sub.face_indices.clone(),
                                texture_name: sub.texture_name.clone(),
                            },
                        ));
                    }
                });
            }
        })
        .id();

    // Build outdoor clickable and occluder faces from BSP model faces.
    {
        let mut outdoor_clickable = Vec::new();
        let mut outdoor_occluders = Vec::new();
        for model in &prepared.map.bsp_models {
            for face in &model.faces {
                if face.vertices_count < 3 || face.is_invisible() {
                    continue;
                }
                let vc = face.vertices_count as usize;
                let verts: Vec<Vec3> = (0..vc)
                    .filter_map(|i| {
                        let idx = face.vertices_ids[i] as usize;
                        model.vertices.get(idx).map(|v| Vec3::from(*v))
                    })
                    .collect();
                if verts.len() < 3 {
                    continue;
                }
                let nx = face.plane.normal[0] as f32 / 65536.0;
                let ny = face.plane.normal[2] as f32 / 65536.0;
                let nz = -face.plane.normal[1] as f32 / 65536.0;
                let normal = Vec3::new(nx, ny, nz);
                let plane_dist = normal.dot(verts[0]);
                if face.cog_trigger_id != 0 {
                    outdoor_clickable.push(crate::game::blv::ClickableFaceInfo {
                        face_index: 0,
                        event_id: face.cog_trigger_id,
                        normal,
                        plane_dist,
                        vertices: verts.clone(),
                    });
                }
                outdoor_occluders.push(crate::game::blv::OccluderFaceInfo {
                    normal,
                    plane_dist,
                    vertices: verts,
                });
            }
        }
        if !outdoor_clickable.is_empty() {
            commands.insert_resource(crate::game::blv::ClickableFaces {
                faces: outdoor_clickable,
                is_indoor: false,
            });
        }
        if !outdoor_occluders.is_empty() {
            commands.insert_resource(crate::game::blv::OccluderFaces {
                faces: outdoor_occluders,
            });
        }
    }

    let decorations = prepared.decorations.clone();
    let actors = prepared.resolved_actors.take();
    let monsters = prepared.resolved_monsters.take();

    // Sort spawn order by distance from player (closest first)
    let player_spawn = Vec3::new(
        save_data.player.position[0],
        save_data.player.position[1],
        save_data.player.position[2],
    );

    let bb_order = sort_by_distance_mm6(
        decorations.entries(),
        player_spawn,
        |d| d.position[0] as f32,
        |d| d.position[1] as f32,
    );

    let actor_order = if let Some(ref a) = actors {
        sort_by_distance_mm6(
            a.get_actors(),
            player_spawn,
            |actor| actor.position[0] as f32,
            |actor| actor.position[1] as f32,
        )
    } else {
        Vec::new()
    };

    let monster_order = if let Some(ref m) = monsters {
        sort_by_distance_mm6(
            m.entries(),
            player_spawn,
            |mon| mon.spawn_position[0] as f32,
            |mon| mon.spawn_position[1] as f32,
        )
    } else {
        Vec::new()
    };

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
        idx: 0,
        frames_elapsed: 0,
        sprite_cache: prepared.sprite_cache.clone(),
        dec_sprite_cache: prepared.dec_sprite_cache.clone(),
        decorations,
        actors,
        monsters,
        monster_order,
        terrain_entity,
    });
}

/// Sort indices by distance from player using MM6 coords (works with i16 or i32).
fn sort_by_distance_mm6<T>(
    items: &[T],
    player: Vec3,
    pos_x: impl Fn(&T) -> f32,
    pos_y: impl Fn(&T) -> f32,
) -> Vec<usize> {
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

    // One-time summary on first frame — useful for diagnosing missing actor/monster spawns
    if p.frames_elapsed == 1 {
        let n_ddm_npcs = p.actors.as_ref().map(|a| a.get_npcs().count()).unwrap_or(0);
        warn!(
            "lazy_spawn: {} decorations, {} DDM NPCs, {} ODM monsters",
            bb_len, n_ddm_npcs, monster_len
        );
    }

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
            let (dirs, dir_masks, px_w, px_h) = sprites::load_decoration_directions(
                &dec.sprite_name,
                game_assets.lod_manager(),
                &mut images,
                &mut materials,
                &mut Some(&mut p.sprite_cache),
            );
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
                let state_masks = vec![vec![dir_masks]];
                let child_id = commands
                    .spawn((
                        Name::new(format!("decoration:{}", key)),
                        Mesh3d(quad),
                        MeshMaterial3d(initial_mat),
                        Transform::from_translation(pos),
                        crate::game::entities::WorldEntity,
                        crate::game::entities::EntityKind::Decoration,
                        crate::game::entities::Billboard,
                        crate::game::entities::AnimationState::Idle,
                        sprites::SpriteSheet::new(states, vec![(sw, sh)], state_masks),
                        crate::game::entities::FacingYaw(dec.facing_yaw),
                    ))
                    .id();
                commands.entity(terrain_entity).add_child(child_id);
                if dec.event_id > 0 {
                    commands
                        .entity(child_id)
                        .insert(crate::game::interaction::DecorationInfo {
                            event_id: dec.event_id as u16,
                            position: pos,
                            billboard_index: dec.billboard_index,
                            declist_id: dec.declist_id,
                            ground_y: dec_pos.y,
                            half_w: 0.0,
                            half_h: 0.0,
                            mask: None,
                        });
                }
                if dec.trigger_radius > 0 && dec.event_id > 0 {
                    commands
                        .entity(child_id)
                        .insert(crate::game::interaction::DecorationTrigger::new(
                            dec.event_id as u16,
                            dec.trigger_radius as f32,
                        ));
                }
                if dec.light_radius > 0 {
                    let light_id = commands.spawn(decoration_point_light(dec.light_radius)).id();
                    commands.entity(child_id).add_child(light_id);
                    commands.entity(child_id).insert(crate::game::entities::SelfLit);
                }
                spawned += 1;
            } else {
                continue;
            }
        } else if dec.num_frames > 1 {
            // Animated non-directional decoration: build a SpriteSheet so update_sprite_sheets
            // handles frame cycling and camera-facing rotation.
            let frame_sprites = bb_mgr.get_animation_frames(game_assets.lod_manager(), key, dec.declist_id);
            if frame_sprites.is_empty() {
                warn!("Animated decoration '{}' has no loadable frames, skipping", key);
                continue;
            }
            let (w, h) = frame_sprites[0].dimensions();
            if w == 0.0 || h == 0.0 {
                continue;
            }
            let quad = meshes.add(Rectangle::new(w, h));
            let pos = dec_pos + Vec3::new(0.0, h / 2.0, 0.0);

            // Build per-frame materials and masks; replicate across 5 directions (non-directional).
            let mut frame_mats: Vec<[Handle<StandardMaterial>; 5]> = vec![];
            let mut frame_masks: Vec<[std::sync::Arc<crate::game::entities::sprites::AlphaMask>; 5]> = vec![];
            for sprite in &frame_sprites {
                let rgba = sprite.image.to_rgba8();
                let msk = std::sync::Arc::new(crate::game::entities::sprites::AlphaMask::from_image(&rgba));
                let tex = images.add(crate::assets::dynamic_to_bevy_image(image::DynamicImage::ImageRgba8(
                    rgba,
                )));
                let mat = materials.add(StandardMaterial {
                    unlit: true,
                    base_color_texture: Some(tex),
                    alpha_mode: AlphaMode::Mask(0.5),
                    cull_mode: None,
                    double_sided: true,
                    perceptual_roughness: 1.0,
                    reflectance: 0.0,
                    ..default()
                });
                frame_mats.push(std::array::from_fn(|_| mat.clone()));
                frame_masks.push(std::array::from_fn(|_| msk.clone()));
            }

            let initial_mat = frame_mats[0][0].clone();
            let mut sheet = sprites::SpriteSheet::new(vec![frame_mats], vec![(w, h)], vec![frame_masks]);
            sheet.frame_duration = dec.frame_duration;

            let child_id = commands
                .spawn((
                    Name::new(format!("decoration:{}", key)),
                    Mesh3d(quad),
                    MeshMaterial3d(initial_mat),
                    Transform::from_translation(pos),
                    crate::game::entities::WorldEntity,
                    crate::game::entities::EntityKind::Decoration,
                    crate::game::entities::Billboard,
                    crate::game::entities::AnimationState::Idle,
                    sheet,
                ))
                .id();
            commands.entity(terrain_entity).add_child(child_id);
            if dec.event_id > 0 {
                commands
                    .entity(child_id)
                    .insert(crate::game::interaction::DecorationInfo {
                        event_id: dec.event_id as u16,
                        position: pos,
                        billboard_index: dec.billboard_index,
                        declist_id: dec.declist_id,
                        ground_y: dec_pos.y,
                        half_w: 0.0,
                        half_h: 0.0,
                        mask: None, // SpriteSheet.current_mask handles this
                    });
            }
            if dec.trigger_radius > 0 && dec.event_id > 0 {
                commands
                    .entity(child_id)
                    .insert(crate::game::interaction::DecorationTrigger::new(
                        dec.event_id as u16,
                        dec.trigger_radius as f32,
                    ));
            }
            // Animated decorations do NOT get DecorFlicker — frame cycling is the visual.
            if dec.light_radius > 0 {
                let light_id = commands.spawn(decoration_point_light(dec.light_radius)).id();
                commands.entity(child_id).add_child(light_id);
                commands.entity(child_id).insert(crate::game::entities::SelfLit);
            }
            spawned += 1;
        } else {
            // Static single-frame decoration.
            let (mat, quad, w, h, mask) = if let Some((m, q, w, h, msk)) = p.dec_sprite_cache.get(key) {
                (m.clone(), q.clone(), *w, *h, msk.clone())
            } else {
                let sprite = match bb_mgr.get(game_assets.lod_manager(), key, dec.declist_id) {
                    Some(s) => s,
                    None => {
                        warn!("Billboard '{}' sprite not found, skipping", key);
                        continue;
                    }
                };
                let (w, h) = sprite.dimensions();
                let rgba = sprite.image.to_rgba8();
                let msk = std::sync::Arc::new(crate::game::entities::sprites::AlphaMask::from_image(&rgba));
                let bevy_img = crate::assets::dynamic_to_bevy_image(image::DynamicImage::ImageRgba8(rgba));
                let tex = images.add(bevy_img);
                let m = materials.add(StandardMaterial {
                    unlit: true,
                    base_color_texture: Some(tex),
                    alpha_mode: AlphaMode::Mask(0.5),
                    cull_mode: None,
                    double_sided: true,
                    perceptual_roughness: 1.0,
                    reflectance: 0.0,
                    ..default()
                });
                let q = meshes.add(Rectangle::new(w, h));
                p.dec_sprite_cache
                    .insert(key.clone(), (m.clone(), q.clone(), w, h, msk.clone()));
                (m, q, w, h, msk)
            };
            let pos = dec_pos + Vec3::new(0.0, h / 2.0, 0.0);
            let child_id = commands
                .spawn((
                    Name::new("decoration"),
                    Mesh3d(quad),
                    MeshMaterial3d(mat),
                    Transform::from_translation(pos),
                    crate::game::entities::WorldEntity,
                    crate::game::entities::EntityKind::Decoration,
                    crate::game::entities::Billboard,
                ))
                .id();
            commands.entity(terrain_entity).add_child(child_id);
            if dec.event_id > 0 {
                commands
                    .entity(child_id)
                    .insert(crate::game::interaction::DecorationInfo {
                        event_id: dec.event_id as u16,
                        position: pos,
                        billboard_index: dec.billboard_index,
                        declist_id: dec.declist_id,
                        ground_y: dec_pos.y,
                        half_w: w / 2.0,
                        half_h: h / 2.0,
                        mask: Some(mask),
                    });
            }
            if dec.trigger_radius > 0 && dec.event_id > 0 {
                commands
                    .entity(child_id)
                    .insert(crate::game::interaction::DecorationTrigger::new(
                        dec.event_id as u16,
                        dec.trigger_radius as f32,
                    ));
            }
            if dec.flicker_rate > 0.0 {
                let phase = (pos.x * 0.137 + pos.z * 0.031).abs().fract();
                commands
                    .entity(child_id)
                    .insert(crate::game::entities::DecorFlicker::new(dec.flicker_rate, phase));
            }
            if dec.light_radius > 0 {
                let light_id = commands.spawn(decoration_point_light(dec.light_radius)).id();
                commands.entity(child_id).add_child(light_id);
                commands.entity(child_id).insert(crate::game::entities::SelfLit);
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

        // DDM monsters (npc_id==0): spawn hostile with MonsterInteractable.
        if actor.is_monster() {
            warn!(
                "Spawning DDM monster '{}' monlist_id={} sprite='{}' variant={} pos={:?}",
                actor.name, actor.monlist_id, actor.standing_sprite, actor.variant, actor.position
            );
            let (states, state_masks, raw_w, raw_h) = sprites::load_entity_sprites(
                &actor.standing_sprite,
                &actor.walking_sprite,
                &actor.attacking_sprite,
                &actor.dying_sprite,
                game_assets.lod_manager(),
                &mut images,
                &mut materials,
                &mut Some(&mut p.sprite_cache),
                actor.variant,
                actor.palette_id,
            );
            if states.is_empty() || states[0].is_empty() {
                error!(
                    "Monster '{}' monlist_id={} sprite '{}' failed to load — skipping",
                    actor.name, actor.monlist_id, actor.standing_sprite
                );
                continue;
            }
            let dsft_scale = bb_mgr.dsft_scale_for_group(&actor.standing_sprite);
            let sw = raw_w * dsft_scale;
            let sh = raw_h * dsft_scale;
            let state_count = states.len();
            let initial_mat = states[0][0][0].clone();
            let quad = meshes.add(Rectangle::new(sw, sh));
            let wx = actor.position[0] as f32;
            let wz = -(actor.position[1] as f32);
            let terrain_y = crate::game::collision::probe_ground_height(&prepared.map.height_map[..], None, wx, wz);
            let ddm_y = actor.position[2] as f32;
            let gy = terrain_y.max(ddm_y);
            let pos = Vec3::new(wx, gy + sh / 2.0, wz);
            commands.entity(terrain_entity).with_child((
                Name::new(format!("monster:{}", actor.name)),
                Mesh3d(quad),
                MeshMaterial3d(initial_mat),
                Transform::from_translation(pos),
                crate::game::entities::WorldEntity,
                crate::game::entities::EntityKind::Monster,
                crate::game::entities::AnimationState::Idle,
                sprites::SpriteSheet::new(states, vec![(sw, sh); state_count], state_masks),
                crate::game::interaction::MonsterInteractable {
                    name: actor.name.clone(),
                },
                crate::game::monster_ai::MonsterAiMode::Wander,
                actor::Actor {
                    name: actor.name.clone(),
                    hp: actor.hp,
                    max_hp: actor.hp,
                    move_speed: actor.move_speed as f32,
                    initial_position: pos,
                    guarding_position: pos,
                    tether_distance: actor.tether_distance as f32,
                    wander_timer: (pos.x * 0.011 + pos.z * 0.017).abs().fract() * 4.0,
                    wander_target: pos,
                    facing_yaw: 0.0,
                    hostile: true,
                    variant: actor.variant,
                    sound_ids: actor.sound_ids,
                    fidget_timer: (pos.x * 0.013 + pos.z * 0.019).abs().fract() * 15.0 + 5.0,
                    attack_range: actor.radius as f32 * 2.0,
                    attack_timer: (pos.x * 0.007 + pos.z * 0.023).abs().fract() * 3.0 + 1.0,
                    attack_anim_remaining: 0.0,
                    ddm_id: i as i32,
                    group_id: actor.group,
                    aggro_range: actor.aggro_range,
                    recovery_secs: actor.recovery_secs,
                    sprite_half_height: sh / 2.0,
                    can_fly: actor.can_fly,
                    ai_type: actor.ai_type.clone(),
                },
            ));
            spawned += 1;
            continue;
        }

        let (s2, m2, w2, h2) = sprites::load_entity_sprites(
            &actor.standing_sprite,
            &actor.walking_sprite,
            &actor.attacking_sprite,
            "",
            game_assets.lod_manager(),
            &mut images,
            &mut materials,
            &mut Some(&mut p.sprite_cache),
            actor.variant,
            actor.palette_id,
        );
        if s2.is_empty() || s2[0].is_empty() {
            error!(
                "NPC '{}' monlist_id={} sprite '{}'/'{}'  failed to load",
                actor.name, actor.monlist_id, actor.standing_sprite, actor.walking_sprite
            );
            continue;
        }
        // Apply DSFT scale (same as decorations — sprite group name lookup)
        let dsft_scale = bb_mgr.dsft_scale_for_group(&actor.standing_sprite);
        let states = s2;
        let state_masks = m2;
        let sw = w2 * dsft_scale;
        let sh = h2 * dsft_scale;
        let state_count = states.len();
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
            let generated_id = crate::game::events::GENERATED_NPC_ID_BASE + i as i32;
            // Pick a complete identity (name + portrait + profession_id) from npcdata.txt peasant
            // entries, split by sex. Falls back to npcnames.txt name + generic portrait if unavailable.
            let (name, portrait, npc_profession_id) = map_events
                .as_ref()
                .and_then(|me| me.npc_table.as_ref())
                .and_then(|t| t.peasant_identity(actor.is_female, i))
                .map(|(n, p, prof)| (n.to_string(), p, prof))
                .unwrap_or_else(|| {
                    let name = map_events
                        .as_ref()
                        .and_then(|me| me.name_pool.as_ref())
                        .map(|pool| pool.name_for(actor.is_female, i).to_string())
                        .unwrap_or_else(|| actor.name.clone());
                    (name, 1, 52) // 52 = "Peasant" in npcprof.txt
                });
            if let Some(ref mut me) = map_events {
                me.generated_npcs.insert(
                    generated_id,
                    lod::game::npc::GeneratedNpc {
                        name: name.clone(),
                        portrait,
                        profession_id: npc_profession_id,
                    },
                );
            }
            (name, generated_id)
        } else {
            // Named NPC: name already resolved in Actor
            (actor.name.clone(), actor.npc_id() as i32)
        };

        // Hover/status text shows the actor TYPE — generic category, never a personal name.
        // Peasants → "Peasant". Quest NPCs → first name from npcdata.txt.
        // Personal name + profession are shown only in the dialogue HUD on click.
        let hover_name = if actor.is_peasant {
            "Peasant".to_string()
        } else {
            display_name
                .split_whitespace()
                .next()
                .unwrap_or(&display_name)
                .to_string()
        };

        commands.entity(terrain_entity).with_child((
            Name::new(format!("npc:{}", actor.name)),
            Mesh3d(quad),
            MeshMaterial3d(initial_mat),
            Transform::from_translation(pos),
            crate::game::entities::WorldEntity,
            crate::game::entities::EntityKind::Npc,
            crate::game::entities::AnimationState::Idle,
            sprites::SpriteSheet::new(states, vec![(sw, sh); state_count], state_masks),
            crate::game::monster_ai::MonsterAiMode::Wander,
            actor::Actor {
                name: actor.name.clone(),
                hp: actor.hp,
                max_hp: actor.hp,
                move_speed: actor.move_speed as f32,
                initial_position: pos,
                guarding_position: pos,
                tether_distance: actor.tether_distance as f32,
                wander_timer: (pos.x * 0.011 + pos.z * 0.017).abs().fract() * 4.0,
                wander_target: pos,
                facing_yaw: 0.0,
                hostile: false,
                variant: actor.variant,
                sound_ids: actor.sound_ids,
                fidget_timer: (pos.x * 0.013 + pos.z * 0.019).abs().fract() * 15.0 + 5.0,
                attack_range: actor.radius as f32 * 2.0,
                attack_timer: (pos.x * 0.007 + pos.z * 0.023).abs().fract() * 3.0 + 1.0,
                attack_anim_remaining: 0.0,
                ddm_id: i as i32,
                group_id: actor.group,
                aggro_range: actor.aggro_range,
                recovery_secs: actor.recovery_secs,
                sprite_half_height: sh / 2.0,
                can_fly: actor.can_fly,
                ai_type: actor.ai_type.clone(),
            },
            crate::game::interaction::NpcInteractable {
                name: hover_name,
                npc_id: effective_npc_id as i16,
            },
        ));
        spawned += 1;
    }

    // ODM spawn-point monsters (outdoor only)
    while monster_idx < monster_len && spawned < batch_max && start.elapsed().as_secs_f32() * 1000.0 < time_budget {
        let i = p.monster_order[monster_idx];
        monster_idx += 1;
        p.idx += 1;

        let mon = match p.monsters.as_ref().and_then(|m| m.entries().get(i)) {
            Some(m) => m,
            None => continue,
        };

        let (states, state_masks, raw_w, raw_h) = sprites::load_entity_sprites(
            &mon.standing_sprite,
            &mon.walking_sprite,
            &mon.attacking_sprite,
            &mon.dying_sprite,
            game_assets.lod_manager(),
            &mut images,
            &mut materials,
            &mut Some(&mut p.sprite_cache),
            mon.variant,
            mon.palette_id,
        );
        if states.is_empty() || states[0].is_empty() {
            error!(
                "ODM monster '{}' sprite '{}' failed to load — skipping",
                mon.name, mon.standing_sprite
            );
            continue;
        }

        let dsft_scale = bb_mgr.dsft_scale_for_group(&mon.standing_sprite);
        let sw = raw_w * dsft_scale;
        let sh = raw_h * dsft_scale;
        let state_count = states.len();
        let initial_mat = states[0][0][0].clone();
        let quad = meshes.add(Rectangle::new(sw, sh));

        // Spread group members around the spawn point center using golden angle distribution.
        let angle = mon.group_index as f32 * 2.399_f32; // ~137.5° golden angle in radians
        let r = mon.spawn_radius as f32;
        let wx = mon.spawn_position[0] as f32 + r * angle.cos();
        let wz = -(mon.spawn_position[1] as f32 + r * angle.sin());
        let terrain_y = crate::game::collision::probe_ground_height(&prepared.map.height_map[..], None, wx, wz);
        let pos = Vec3::new(wx, terrain_y + sh / 2.0, wz);

        commands.entity(terrain_entity).with_child((
            Name::new(format!("monster:{}", mon.name)),
            Mesh3d(quad),
            MeshMaterial3d(initial_mat),
            Transform::from_translation(pos),
            crate::game::entities::WorldEntity,
            crate::game::entities::EntityKind::Monster,
            crate::game::entities::AnimationState::Idle,
            sprites::SpriteSheet::new(states, vec![(sw, sh); state_count], state_masks),
            crate::game::interaction::MonsterInteractable { name: mon.name.clone() },
            crate::game::monster_ai::MonsterAiMode::Wander,
            actor::Actor {
                name: mon.name.clone(),
                hp: mon.hp,
                max_hp: mon.hp,
                move_speed: mon.move_speed as f32,
                initial_position: pos,
                guarding_position: pos,
                tether_distance: mon.radius as f32 * 2.0,
                wander_timer: (pos.x * 0.011 + pos.z * 0.017).abs().fract() * 4.0,
                wander_target: pos,
                facing_yaw: 0.0,
                hostile: true,
                variant: mon.variant,
                sound_ids: mon.sound_ids,
                fidget_timer: (pos.x * 0.013 + pos.z * 0.019).abs().fract() * 15.0 + 5.0,
                attack_range: mon.body_radius as f32 * 2.0,
                attack_timer: (pos.x * 0.007 + pos.z * 0.023).abs().fract() * 3.0 + 1.0,
                attack_anim_remaining: 0.0,
                ddm_id: -1, // ODM spawn group — no DDM actor index
                group_id: 0,
                aggro_range: mon.aggro_range,
                recovery_secs: mon.recovery_secs,
                sprite_half_height: sh / 2.0,
                can_fly: mon.can_fly,
                ai_type: mon.ai_type.clone(),
            },
        ));
        spawned += 1;
    }

    progress.done = p.idx;

    if p.idx >= bb_len + actor_len + monster_len {
        commands.remove_resource::<PendingSpawns>();
    }
}

/// Build a `PointLight` for a decoration with the given MM6 light radius.
///
/// Intensity scales with radius² so smaller lights aren't washed out by larger ones.
/// Color is warm orange (torches, braziers). Shadows disabled for performance.
/// Flicker is free: the light entity inherits visibility from its parent decoration.
pub(crate) fn decoration_point_light(light_radius: u16) -> impl Bundle {
    let range = light_radius as f32;
    // Intensity scaled so a radius-512 torch (~medium MM6 torch) reaches ~100k lux at
    // 1m, which is visible against near-zero indoor ambient.
    // Formula: range² * 0.4 ≈ 100k for range=512.
    PointLight {
        color: Color::srgb(1.0, 0.78, 0.40),
        intensity: range * range * 200.0,
        range,
        shadows_enabled: false,
        ..default()
    }
}

/// Handle `ApplyTextureOutdoors` messages — swap the material on the matching BSP sub-mesh entity.
fn apply_texture_outdoors(
    mut events: MessageReader<ApplyTextureOutdoors>,
    mut query: Query<(&mut BspSubMesh, &mut MeshMaterial3d<StandardMaterial>)>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    game_assets: Res<GameAssets>,
    cfg: Res<crate::config::GameConfig>,
) {
    for ev in events.read() {
        let Some((mut sub, mut mat_handle)) = query
            .iter_mut()
            .find(|(sub, _)| sub.model_index == ev.model && sub.face_indices.contains(&ev.facet))
        else {
            warn!(
                "SetTextureOutdoors: no sub-mesh found for model={} facet={}",
                ev.model, ev.facet
            );
            continue;
        };

        let Some(img) = game_assets.game_lod().bitmap(&ev.texture_name) else {
            warn!("SetTextureOutdoors: texture '{}' not found in LOD", ev.texture_name);
            continue;
        };

        let mut image = crate::assets::dynamic_to_bevy_image(img);
        image.sampler = crate::assets::sampler_for_filtering(&cfg.models_filtering);
        let tex_handle = images.add(image);

        let new_mat = StandardMaterial {
            base_color: Color::srgb(1.8, 1.8, 1.8),
            base_color_texture: Some(tex_handle),
            alpha_mode: AlphaMode::Opaque,
            cull_mode: None,
            double_sided: true,
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            metallic: 0.0,
            ..default()
        };
        mat_handle.0 = materials.add(new_mat);
        sub.texture_name = ev.texture_name.clone();

        info!(
            "SetTextureOutdoors: model={} facet={} → '{}'",
            ev.model, ev.facet, ev.texture_name
        );
    }
}
