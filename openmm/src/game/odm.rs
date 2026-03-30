use bevy::prelude::*;
use bevy::render::render_resource::Face;

use crate::GameState;
use crate::game::InGame;
use crate::assets::GameAssets;
use crate::game::entities::{actor, sprites};
use crate::game::terrain_material::{TerrainMaterial, WaterExtension};

/// Marker for the map music entity, so we can despawn it on map change.
#[derive(Component)]
struct MapMusic;

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
    resolved_monsters: Vec<crate::states::loading::PreparedMonster>,
    /// NPC sprite lookup: monlist_id → (standing_root, walking_root, palette_id)
    npc_sprite_table: std::collections::HashMap<u8, (String, String, u16)>,
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
    });
    game_state.set(GameState::Loading);
}

fn spawn_world(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut terrain_materials: ResMut<Assets<TerrainMaterial>>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    prepared: Option<Res<PreparedWorld>>,
    game_assets: Res<GameAssets>,
    save_data: Res<crate::save::GameSave>,
    cfg: Res<crate::config::GameConfig>,
    world_state: Res<crate::game::world_state::WorldState>,
    existing_music: Query<Entity, With<MapMusic>>,
) {
    let Some(prepared) = prepared else {
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
    let terrain_sampler = if cfg.terrain_filtering == "nearest" {
        crate::assets::nearest_sampler()
    } else {
        crate::assets::repeat_linear_sampler()
    };
    terrain_texture.sampler = terrain_sampler;
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
    let water_sampler = if cfg.terrain_filtering == "nearest" {
        crate::assets::repeat_sampler()
    } else {
        crate::assets::repeat_linear_sampler()
    };
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

                let model_sampler = if cfg.models_filtering == "nearest" {
                    crate::assets::nearest_sampler()
                } else {
                    crate::assets::repeat_linear_sampler()
                };
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

    // Resolve monsters from spawn points using current map (not save_data which may lag)
    let resolved_monsters = resolve_monsters(&prepared, &game_assets, &world_state.map.name);

    // Build NPC sprite lookup from monlist — each DDM actor has a monlist_id
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

    let total = bb_order.len() + actor_order.len() + monster_order.len();
    // Stop any existing music from previous map
    for entity in existing_music.iter() {
        commands.entity(entity).despawn();
    }

    // Play map music
    if cfg.music_volume > 0.0 && prepared.music_track > 0 {
        let data_path = lod::get_data_path();
        let music_path = std::path::Path::new(&data_path).join(format!("Music/{}.mp3", prepared.music_track));
        if let Ok(bytes) = std::fs::read(&music_path) {
            let source = AudioSource { bytes: bytes.into() };
            let handle = audio_sources.add(source);
            commands.spawn((
                AudioPlayer(handle),
                PlaybackSettings {
                    mode: bevy::audio::PlaybackMode::Loop,
                    volume: bevy::audio::Volume::Linear(cfg.music_volume),
                    ..default()
                },
                MapMusic,
                InGame,
            ));
            info!("Playing music track {} (vol={:.1})", prepared.music_track, cfg.music_volume);
        } else {
            warn!("Music file not found: {:?}", music_path);
        }
    }

    commands.insert_resource(SpawnProgress { total, done: 0 });
    commands.insert_resource(PendingSpawns {
        billboard_order: bb_order,
        actor_order,
        monster_order,
        idx: 0,
        frames_elapsed: 0,
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
    map_name: &crate::game::map_name::MapName,
) -> Vec<crate::states::loading::PreparedMonster> {
    let mut monsters = Vec::new();
    let Ok(mapstats) = lod::mapstats::MapStats::new(game_assets.lod_manager()) else { return monsters };
    let Ok(monlist) = lod::monlist::MonsterList::new(game_assets.lod_manager()) else { return monsters };

    let map_config = mapstats.get(&map_name.to_string());
    let Some(cfg) = map_config else { return monsters };

    for sp in &prepared.map.spawn_points {
        let group_size = 3 + ((sp.position[0].unsigned_abs() + sp.position[1].unsigned_abs()) % 3) as i32;
        for g in 0..group_size {
            // Each monster in the group gets its own A/B/C roll (matching original engine).
            // Seed is per-monster (position + group index) for deterministic results.
            let seed = (sp.position[0].unsigned_abs() + sp.position[1].unsigned_abs() + g as u32) as u32;
            let Some((mon_name, dif)) = cfg.monster_for_index(sp.monster_index, seed) else { continue };
            let Some(desc) = monlist.find_with_sprite(mon_name, dif, game_assets.lod_manager()) else { continue };

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

/// Build a lookup table: monlist_id → (standing_sprite, walking_sprite) from monlist.
/// Resolves monlist sprite group names through the DSFT to get actual sprite file
/// names and palette IDs. The DSFT is the authoritative mapping from monlist names
/// to LOD sprite files.
pub fn build_npc_sprite_table(game_assets: &GameAssets) -> std::collections::HashMap<u8, (String, String, u16)> {
    let mut table = std::collections::HashMap::new();
    let Ok(monlist) = lod::monlist::MonsterList::new(game_assets.lod_manager()) else {
        return table;
    };
    let Ok(dsft) = lod::dsft::DSFT::new(game_assets.lod_manager()) else {
        return table;
    };
    let lod = game_assets.lod_manager();

    for (i, desc) in monlist.monsters.iter().enumerate() {
        if i > 255 { break; }
        let st_group = &desc.sprite_names[0];
        let wa_group = &desc.sprite_names[1];
        if st_group.is_empty() { continue; }

        // Resolve standing sprite through DSFT: group name → (actual sprite file, palette_id)
        let st_resolved = resolve_dsft_sprite(&dsft, st_group, lod);
        let wa_resolved = resolve_dsft_sprite(&dsft, wa_group, lod);

        if let Some((st_name, palette_id)) = st_resolved {
            let wa_name = wa_resolved.map(|(n, _)| n).unwrap_or_else(|| st_name.clone());
            table.insert(i as u8, (st_name, wa_name, palette_id as u16));
        }
    }
    table
}

/// Resolve a monlist sprite group name through the DSFT to find the actual
/// sprite file name that exists in the LOD.
/// Resolve a monlist sprite group name through the DSFT to find the actual
/// sprite file name and palette_id. Returns (sprite_root, palette_id).
fn resolve_dsft_sprite(dsft: &lod::dsft::DSFT, group_name: &str, lod: &lod::LodManager) -> Option<(String, i16)> {
    for frame in &dsft.frames {
        if let Some(gname) = frame.group_name() {
            if gname.eq_ignore_ascii_case(group_name) {
                if let Some(sprite_name) = frame.sprite_name() {
                    // DSFT sprite names include the frame letter (e.g., "fmpstaa" = root "fmpsta" + frame "a").
                    // Strip trailing digits AND the frame letter to get the root that the
                    // sprite loader expects (it appends frame letters a-f and direction digits 0-4).
                    let without_digits = sprite_name.trim_end_matches(|c: char| c.is_ascii_digit());
                    // Strip the trailing frame letter (a-f)
                    let root = if without_digits.len() > 1 {
                        let last = without_digits.as_bytes()[without_digits.len() - 1];
                        if last >= b'a' && last <= b'f' {
                            &without_digits[..without_digits.len() - 1]
                        } else {
                            without_digits
                        }
                    } else {
                        without_digits
                    };
                    let test = format!("sprites/{}a0", root.to_lowercase());
                    if lod.try_get_bytes(&test).is_ok() {
                        return Some((root.to_lowercase(), frame.palette_id));
                    }
                }
                break;
            }
        }
    }
    // Fallback: try the group name directly
    let root = group_name.trim_end_matches(|c: char| c.is_ascii_digit());
    let mut try_root = root;
    while try_root.len() >= 3 {
        let test = format!("sprites/{}a0", try_root.to_lowercase());
        if lod.try_get_bytes(&test).is_ok() {
            return Some((try_root.to_lowercase(), 0));
        }
        try_root = &try_root[..try_root.len() - 1];
    }
    None
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
    mut progress: ResMut<SpawnProgress>,
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
    let monster_len = p.resolved_monsters.len();
    let mut bb_idx = p.idx.min(bb_len);
    let mut actor_idx = p.idx.saturating_sub(bb_len).min(actor_len);
    let mut monster_idx = p.idx.saturating_sub(bb_len + actor_len).min(monster_len);

    // Billboards
    while bb_idx < bb_len && spawned < batch_max && start.elapsed().as_secs_f32() * 1000.0 < time_budget {
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
    while actor_idx < actor_len && spawned < batch_max && start.elapsed().as_secs_f32() * 1000.0 < time_budget {
        let i = p.actor_order[actor_idx];
        actor_idx += 1;
        p.idx += 1;
        let a = &prepared.actors[i];
        if a.hp <= 0 || a.position[0].abs() > 20000 || a.position[1].abs() > 20000 { continue; }

        let Some((s, w, pal_id)) = p.npc_sprite_table.get(&a.monlist_id) else {
            error!("NPC '{}' monster_id={} has no sprite in DSFT table — skipping", a.name, a.monlist_id);
            continue;
        };
        // Compute palette variant: base palette is the minimum among same-sprite entries.
        let base_pal = p.npc_sprite_table.values()
            .filter(|(ss, _, _)| ss == s)
            .map(|(_, _, p)| *p)
            .min()
            .unwrap_or(*pal_id);
        let variant = (pal_id - base_pal + 1).min(3) as u8;
        let (s2, w2, h2) = sprites::load_entity_sprites(
            s, w, game_assets.lod_manager(),
            &mut images, &mut materials, &mut Some(&mut p.sprite_cache), variant);
        if s2.is_empty() || s2[0].is_empty() {
            error!("NPC '{}' monster_id={} sprite '{}'/'{}'  failed to load", a.name, a.monlist_id, s, w);
            continue;
        }
        let states = s2;
        let sw = w2;
        let sh = h2;
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
    while monster_idx < monster_len && spawned < batch_max && start.elapsed().as_secs_f32() * 1000.0 < time_budget {
        let m = &p.resolved_monsters[p.monster_order[monster_idx]];
        monster_idx += 1;
        p.idx += 1;
        let (states, sw, sh) = sprites::load_entity_sprites(
            &m.standing_sprite, &m.walking_sprite, game_assets.lod_manager(),
            &mut images, &mut materials, &mut Some(&mut p.sprite_cache), m.variant);
        if states.is_empty() || states[0].is_empty() {
            error!("Monster sprite '{}'/'{}'  failed to load — skipping", m.standing_sprite, m.walking_sprite);
            continue;
        }
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

    progress.done = p.idx;

    if p.idx >= bb_len + actor_len + monster_len {
        commands.remove_resource::<PendingSpawns>();
    }
}
