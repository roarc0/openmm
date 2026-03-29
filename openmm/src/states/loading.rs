use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

use std::collections::HashMap;

use crate::{
    assets::GameAssets,
    config::GameConfig,
    despawn_all,
    game::map_name::MapName,
    game::odm::OdmName,
    GameState,
};
use lod::{
    blv::Blv,
    ddm::{Ddm, DdmActor},
    dtile::{Dtile, TileTable},
    odm::{Odm, OdmData},
};

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Loading), loading_setup)
            .add_systems(Update, loading_step.run_if(in_state(GameState::Loading)))
            .add_systems(OnExit(GameState::Loading), despawn_all::<InLoading>);
    }
}

#[derive(Component)]
struct InLoading;

/// Requested map to load. Insert this resource before transitioning to Loading state.
#[derive(Resource)]
pub struct LoadRequest {
    pub map_name: MapName,
}

/// Tracks which step of the loading pipeline we're on.
#[derive(Resource)]
struct LoadingProgress {
    step: LoadingStep,
    odm: Option<Odm>,
    tile_table: Option<TileTable>,
    odm_data: Option<OdmData>,
    terrain_mesh: Option<Mesh>,
    terrain_texture: Option<Image>,
    water_texture: Option<Image>,
    models: Option<Vec<PreparedModel>>,
    billboards: Option<Vec<PreparedBillboard>>,
    actors: Option<Vec<DdmActor>>,
    monsters: Option<Vec<PreparedMonster>>,
    start_points: Option<Vec<StartPoint>>,
    sprite_cache: Option<crate::game::entities::sprites::SpriteCache>,
    billboard_cache: Option<std::collections::HashMap<String, (Handle<StandardMaterial>, Handle<Mesh>, f32)>>,
    water_cells: Option<Vec<bool>>,
    music_track: u8,
    blv: Option<Blv>,
}

/// Resource for indoor (BLV) maps — the indoor equivalent of PreparedWorld.
#[derive(Resource)]
pub struct PreparedIndoorWorld {
    pub models: Vec<PreparedModel>,
    pub start_points: Vec<StartPoint>,
}

pub struct PreparedModel {
    /// Sub-meshes, one per unique texture in the BSP model.
    pub sub_meshes: Vec<PreparedSubMesh>,
    /// BSP model name (e.g. "TavFrntW", "ArmoryW", "GenStorE").
    pub name: String,
    /// Model center position in Bevy coordinates.
    pub position: Vec3,
    /// Unique event IDs from this model's faces (cog_trigger_id values > 0).
    pub event_ids: Vec<u16>,
}

pub struct PreparedSubMesh {
    pub mesh: Mesh,
    pub material: StandardMaterial,
    pub texture: Option<Image>,
}

/// A monster to spawn from a spawn point, with resolved sprite names.
pub struct PreparedMonster {
    /// Position in MM6 coordinates.
    pub position: [i32; 3],
    pub radius: u16,
    /// Sprite name roots: [standing, walking]
    pub standing_sprite: String,
    pub walking_sprite: String,
    pub height: u16,
    pub move_speed: u16,
    pub hostile: bool,
    /// Difficulty variant: 1=A (base), 2=B, 3=C. Used for color tinting.
    pub variant: u8,
}

pub struct PreparedBillboard {
    /// Position in Bevy coordinates.
    pub position: Vec3,
    /// Decoration name (for sprite lookup).
    pub declist_name: String,
    /// Declist ID for BillboardManager lookup.
    pub declist_id: u16,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum LoadingStep {
    #[default]
    ParseMap,
    BuildTerrain,
    BuildAtlas,
    BuildModels,
    BuildBillboards,
    PreloadSprites,
    Done,
}

impl LoadingStep {
    fn label(&self) -> &'static str {
        match self {
            Self::ParseMap => "Parsing map...",
            Self::BuildTerrain => "Building terrain...",
            Self::BuildAtlas => "Building textures...",
            Self::BuildModels => "Building models...",
            Self::BuildBillboards => "Loading decorations...",
            Self::PreloadSprites => "Loading sprites...",
            Self::Done => "Done!",
        }
    }

    fn next(&self) -> Self {
        match self {
            Self::ParseMap => Self::BuildTerrain,
            Self::BuildTerrain => Self::BuildAtlas,
            Self::BuildAtlas => Self::BuildModels,
            Self::BuildModels => Self::BuildBillboards,
            Self::BuildBillboards => Self::PreloadSprites,
            Self::PreloadSprites => Self::Done,
            Self::Done => Self::Done,
        }
    }
}

/// A named start/teleport point extracted from map decorations.
pub struct StartPoint {
    pub name: String,
    pub position: Vec3,
    pub yaw: f32,
}

/// Resource containing everything needed to spawn the world after loading.
#[derive(Resource)]
pub struct PreparedWorld {
    pub map: Odm,
    pub terrain_mesh: Mesh,
    pub terrain_texture: Image,
    pub water_texture: Option<Image>,
    pub models: Vec<PreparedModel>,
    pub billboards: Vec<PreparedBillboard>,
    pub actors: Vec<DdmActor>,
    pub monsters: Vec<PreparedMonster>,
    pub start_points: Vec<StartPoint>,
    pub sprite_cache: crate::game::entities::sprites::SpriteCache,
    pub billboard_cache: std::collections::HashMap<String, (Handle<StandardMaterial>, Handle<Mesh>, f32)>,
    pub water_cells: Vec<bool>,
    /// Music track ID from mapstats.txt (maps to Music/{track}.mp3). 0 = no music.
    pub music_track: u8,
}

#[derive(Component)]
struct LoadingText;

fn loading_setup(
    mut commands: Commands,
    load_request: Option<Res<LoadRequest>>,
    save_data: Res<crate::save::GameSave>,
    cfg: Res<GameConfig>,
    game_assets: Res<GameAssets>,
    mut ui_assets: ResMut<crate::ui_assets::UiAssets>,
    mut images: ResMut<Assets<Image>>,
) {
    // Priority: LoadRequest (map switching) > config file/CLI > save data
    let map_name = load_request
        .map(|r| r.map_name.clone())
        .or_else(|| {
            cfg.map.as_ref().and_then(|m| {
                MapName::try_from(m.as_str())
                    .inspect_err(|e| eprintln!("warning: invalid map in config: {e}"))
                    .ok()
            })
        })
        .unwrap_or_else(|| MapName::Outdoor(OdmName {
            x: save_data.map.map_x,
            y: save_data.map.map_y,
        }));

    commands.insert_resource(LoadingProgress {
        step: LoadingStep::ParseMap,
        odm: None,
        tile_table: None,
        odm_data: None,
        terrain_mesh: None,
        terrain_texture: None,
        water_texture: None,
        models: None,
        billboards: None,
        actors: None,
        monsters: None,
        start_points: None,
        sprite_cache: None,
        billboard_cache: None,
        water_cells: None,
        music_track: 0,
        blv: None,
    });

    // Keep the load request around as context
    commands.insert_resource(LoadRequest { map_name });

    // Spawn loading screen with loading.pcx background from LOD
    commands.spawn((Camera2d, InLoading));

    let loading_bg = ui_assets.get_or_load("loading.pcx", &game_assets, &mut images, &cfg);

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::FlexEnd,
                justify_content: JustifyContent::Center,
                ..default()
            },
            ImageNode::new(loading_bg.unwrap_or_default()),
            InLoading,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Loading..."),
                TextFont {
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    margin: UiRect::all(Val::Px(20.0)),
                    ..default()
                },
                LoadingText,
            ));
        });
}

fn loading_step(
    mut progress: ResMut<LoadingProgress>,
    game_assets: Res<GameAssets>,
    load_request: Res<LoadRequest>,
    mut game_state: ResMut<NextState<GameState>>,
    mut commands: Commands,
    mut text_query: Query<&mut Text, With<LoadingText>>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Update loading text
    for mut text in &mut text_query {
        **text = progress.step.label().to_string();
    }

    match progress.step {
        LoadingStep::ParseMap => {
            let map_name = load_request.map_name.to_string();
            if load_request.map_name.is_indoor() {
                match Blv::new(game_assets.lod_manager(), &map_name) {
                    Ok(blv) => {
                        progress.blv = Some(blv);
                        // Skip terrain — jump straight to BuildModels
                        progress.step = LoadingStep::BuildModels;
                    }
                    Err(e) => {
                        error!("Failed to parse indoor map {}: {}", map_name, e);
                        return;
                    }
                }
                return;
            }
            match Odm::new(game_assets.lod_manager(), &map_name) {
                Ok(odm) => {
                    match odm.tile_table(game_assets.lod_manager()) {
                        Ok(tile_table) => {
                            // Build water map from tile data
                            if let Ok(dtile) = Dtile::new(game_assets.lod_manager()) {
                                let water_cells: Vec<bool> = odm.tile_map.iter()
                                    .map(|&idx| dtile.is_deep_water_tile(idx))
                                    .collect();
                                progress.water_cells = Some(water_cells);
                            }
                            // Load actors from DDM
                            let actors = Ddm::new(game_assets.lod_manager(), &map_name)
                                .map(|ddm| ddm.actors)
                                .unwrap_or_default();
                            progress.actors = Some(actors);

                            // Store raw spawn data — monsters resolved lazily
                            progress.monsters = Some(Vec::new());

                            progress.tile_table = Some(tile_table);
                            progress.odm = Some(odm);
                            progress.step = progress.step.next();
                        }
                        Err(e) => {
                            error!("Failed to load tile table: {}", e);
                            return;
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to parse map {}: {}", map_name, e);
                    return;
                }
            }
        }
        LoadingStep::BuildTerrain => {
            if let (Some(odm), Some(tile_table)) = (&progress.odm, &progress.tile_table) {
                let odm_data = OdmData::new(odm, tile_table);
                let mut mesh = Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::RENDER_WORLD,
                );
                mesh.insert_indices(Indices::U32(odm_data.indices.clone()));
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, odm_data.positions.clone());
                mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, odm_data.normals.clone());
                mesh.duplicate_vertices();
                mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, odm_data.uvs.clone());
                _ = mesh.generate_tangents();

                progress.terrain_mesh = Some(mesh);
                progress.odm_data = Some(odm_data);
                progress.step = progress.step.next();
            }
        }
        LoadingStep::BuildAtlas => {
            if let Some(tile_table) = &progress.tile_table {
                match tile_table.atlas_image(game_assets.lod_manager()) {
                    Ok(atlas) => {
                        progress.terrain_texture = Some(crate::assets::dynamic_to_bevy_image(atlas));

                        // Load water texture
                        progress.water_texture = game_assets
                            .lod_manager()
                            .bitmap("wtrtyl")
                            .map(|img| {
                                let mut water_img = crate::assets::dynamic_to_bevy_image(img);
                                water_img.sampler = crate::assets::repeat_sampler();
                                water_img
                            });

                        progress.step = progress.step.next();
                    }
                    Err(e) => {
                        error!("Failed to build atlas: {}", e);
                        return;
                    }
                }
            }
        }
        LoadingStep::BuildModels => {
            if let Some(blv) = &progress.blv {
                // Indoor: build meshes from BLV faces
                let mut texture_sizes: HashMap<String, (u32, u32)> = HashMap::new();
                for name in &blv.texture_names {
                    if name.is_empty() || texture_sizes.contains_key(name) { continue; }
                    if let Some(img) = game_assets.lod_manager().bitmap(name) {
                        texture_sizes.insert(name.clone(), (img.width(), img.height()));
                    }
                }
                let textured = blv.textured_meshes(&texture_sizes);
                let models = vec![PreparedModel {
                    sub_meshes: textured.into_iter().map(|tm| {
                        let mut mesh = Mesh::new(
                            PrimitiveTopology::TriangleList,
                            RenderAssetUsages::RENDER_WORLD,
                        );
                        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, tm.positions);
                        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, tm.normals);
                        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, tm.uvs);
                        _ = mesh.generate_tangents();
                        let texture = game_assets.lod_manager().bitmap(&tm.texture_name)
                            .map(|img| {
                                let mut image = crate::assets::dynamic_to_bevy_image(img);
                                image.sampler = crate::assets::repeat_sampler();
                                image
                            });
                        PreparedSubMesh {
                            mesh,
                            material: StandardMaterial {
                                base_color: Color::WHITE,
                                alpha_mode: AlphaMode::Opaque,
                                cull_mode: None,
                                double_sided: true,
                                perceptual_roughness: 1.0,
                                reflectance: 0.0,
                                metallic: 0.0,
                                ..default()
                            },
                            texture,
                        }
                    }).collect(),
                    name: "blv_faces".to_string(),
                    position: Vec3::ZERO,
                    event_ids: vec![],
                }];
                // Compute spawn from sector 1's bounding box (sector 0 is the void).
                // Use XY center, Z at floor level (min Z in MM6 = lowest point).
                let spawn_sector = blv.sectors.get(1).or_else(|| blv.sectors.first());
                let spawn_pos = if let Some(sector) = spawn_sector {
                    let cx = ((sector.bbox_min[0] as i32 + sector.bbox_max[0] as i32) / 2) as i32;
                    let cy = ((sector.bbox_min[1] as i32 + sector.bbox_max[1] as i32) / 2) as i32;
                    // Floor level: minimum of the two Z values (bbox may be swapped)
                    let floor_z = sector.bbox_min[2].min(sector.bbox_max[2]) as i32;
                    Vec3::from(lod::odm::mm6_to_bevy(cx, cy, floor_z))
                } else {
                    Vec3::ZERO
                };
                let start_points = vec![StartPoint {
                    name: "sector_center".to_string(),
                    position: spawn_pos,
                    yaw: 0.0,
                }];
                commands.insert_resource(PreparedIndoorWorld { models, start_points });
                commands.remove_resource::<LoadingProgress>();
                game_state.set(GameState::Game);
                return;
            }
            if let Some(odm) = &progress.odm {
                // Collect texture sizes for UV normalization
                let mut texture_sizes: HashMap<String, (u32, u32)> = HashMap::new();
                for b in &odm.bsp_models {
                    for name in &b.texture_names {
                        if !texture_sizes.contains_key(name) {
                            if let Some(img) = game_assets.lod_manager().bitmap(name) {
                                texture_sizes
                                    .insert(name.clone(), (img.width(), img.height()));
                            }
                        }
                    }
                }

                let models = odm
                    .bsp_models
                    .iter()
                    .map(|b| {
                        let textured = b.textured_meshes(&texture_sizes);
                        let sub_meshes = textured
                            .into_iter()
                            .map(|tm| {
                                let mut mesh = Mesh::new(
                                    PrimitiveTopology::TriangleList,
                                    RenderAssetUsages::RENDER_WORLD,
                                );
                                mesh.insert_attribute(
                                    Mesh::ATTRIBUTE_POSITION,
                                    tm.positions,
                                );
                                mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, tm.normals);
                                mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, tm.uvs);
                                _ = mesh.generate_tangents();

                                let texture = game_assets
                                    .lod_manager()
                                    .bitmap(&tm.texture_name)
                                    .map(|img| {
                                        let mut image = crate::assets::dynamic_to_bevy_image(img);
                                        image.sampler = crate::assets::repeat_sampler();
                                        image
                                    });

                                let material = StandardMaterial {
                                    base_color: Color::srgb(1.4, 1.4, 1.4),
                                    alpha_mode: AlphaMode::Opaque,
                                    cull_mode: None,
                                    double_sided: true,
                                    perceptual_roughness: 1.0,
                                    reflectance: 0.0,
                                    metallic: 0.0,
                                    ..default()
                                };

                                PreparedSubMesh {
                                    mesh,
                                    material,
                                    texture,
                                }
                            })
                            .collect();
                        let pos = lod::odm::mm6_to_bevy(
                            b.header.position[0],
                            b.header.position[1],
                            b.header.position[2],
                        );
                        let mut event_ids: Vec<u16> = b.faces.iter()
                            .filter_map(|f| if f.cog_trigger_id > 0 { Some(f.cog_trigger_id) } else { None })
                            .collect();
                        event_ids.sort_unstable();
                        event_ids.dedup();
                        PreparedModel {
                            sub_meshes,
                            name: b.header.name.clone(),
                            position: Vec3::from(pos),
                            event_ids,
                        }
                    })
                    .collect();
                progress.models = Some(models);
                progress.step = progress.step.next();
            }
        }
        LoadingStep::BuildBillboards => {
            // Extract start/teleport points and filter out non-renderable decorations.
            // Decorations with game_name containing "Start" are teleport markers
            // (e.g., "Party Start", "North Start"). They should not render.
            if let Some(odm) = &progress.odm {
                let bb_mgr = lod::billboard::BillboardManager::new(game_assets.lod_manager()).ok();
                let mut start_points = Vec::new();
                let mut billboards = Vec::new();

                for bb in &odm.billboards {
                    if bb.data.is_invisible() { continue; }

                    let pos = Vec3::from(lod::odm::mm6_to_bevy(
                        bb.data.position[0], bb.data.position[1], bb.data.position[2],
                    ));
                    let yaw = bb.data.direction_degrees as f32 * std::f32::consts::PI / 1024.0;

                    // Check if this is a start/teleport marker
                    let is_marker = bb_mgr.as_ref()
                        .and_then(|mgr| mgr.get_declist_item(bb.data.declist_id))
                        .map(|item| item.is_marker() || item.is_no_draw())
                        .unwrap_or(false);

                    let name_lower = bb.declist_name.to_lowercase();
                    let is_start = name_lower.contains("start") || is_marker;

                    if is_start {
                        start_points.push(StartPoint {
                            name: bb.declist_name.clone(),
                            position: pos,
                            yaw,
                        });
                        continue; // Don't render markers
                    }

                    billboards.push(PreparedBillboard {
                        position: pos,
                        declist_name: bb.declist_name.clone(),
                        declist_id: bb.data.declist_id,
                    });
                }

                progress.start_points = Some(start_points);
                progress.billboards = Some(billboards);
                progress.step = progress.step.next();
            }
        }
        LoadingStep::PreloadSprites => {
            // Pre-decode ALL sprite textures during loading screen so that
            // lazy_spawn only does cheap cache lookups during gameplay.
            let mut cache = progress.sprite_cache.take().unwrap_or_default();

            // NPC sprite roots (standing + walking)
            let npc_roots: Vec<(&str, u8)> = crate::game::entities::actor::NPC_SPRITES
                .iter()
                .flat_map(|&(st, wa)| [(st, 0u8), (wa, 0u8)])
                .collect();
            cache.preload(&npc_roots, game_assets.lod_manager(), &mut images, &mut materials);

            // Resolve music track from mapstats
            if let Ok(mapstats) = lod::mapstats::MapStats::new(game_assets.lod_manager()) {
                if let Some(cfg) = mapstats.get(&load_request.map_name.to_string()) {
                    progress.music_track = cfg.music_track;
                }
            }

            // Resolve and preload monster sprites for this map
            if let (Ok(mapstats), Ok(monlist)) = (
                lod::mapstats::MapStats::new(game_assets.lod_manager()),
                lod::monlist::MonsterList::new(game_assets.lod_manager()),
            ) {
                let map_cfg = mapstats.get(&load_request.map_name.to_string());
                if let Some(cfg) = map_cfg {
                    // Collect unique (sprite_root, variant) pairs
                    let mut monster_roots: Vec<(String, u8)> = Vec::new();
                    let mut seen = std::collections::HashSet::new();
                    if let Some(odm) = &progress.odm {
                        for sp in &odm.spawn_points {
                            let seed = (sp.position[0].unsigned_abs() + sp.position[1].unsigned_abs()) as u32;
                            if let Some((mon_name, variant)) = cfg.monster_for_index(sp.monster_index, seed) {
                                if let Some(desc) = monlist.find_by_name(mon_name, variant) {
                                    for name in &desc.sprite_names[..2] {
                                        let key = format!("{}@v{}", name, variant);
                                        if seen.insert(key) {
                                            monster_roots.push((name.clone(), variant));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    let refs: Vec<(&str, u8)> = monster_roots.iter()
                        .map(|(s, v)| (s.as_str(), *v)).collect();
                    cache.preload(&refs, game_assets.lod_manager(), &mut images, &mut materials);
                }
            }

            // Pre-decode billboard/decoration sprites
            let mut bb_cache = progress.billboard_cache.take().unwrap_or_default();
            if let Some(billboards) = &progress.billboards {
                let bb_mgr = lod::billboard::BillboardManager::new(game_assets.lod_manager()).ok();
                if let Some(ref mgr) = bb_mgr {
                    for bb in billboards {
                        if bb_cache.contains_key(&bb.declist_name) { continue; }
                        if let Some(sprite) = mgr.get(game_assets.lod_manager(), &bb.declist_name, bb.declist_id) {
                            let (w, h) = sprite.dimensions();
                            let bevy_img = crate::assets::dynamic_to_bevy_image(sprite.image);
                            let tex = images.add(bevy_img);
                            let m = materials.add(StandardMaterial {
                                base_color_texture: Some(tex),
                                alpha_mode: AlphaMode::Mask(0.5),
                                unlit: true,
                                cull_mode: None, double_sided: true,
                                perceptual_roughness: 1.0, reflectance: 0.0, ..default()
                            });
                            let q = meshes.add(Rectangle::new(w, h));
                            bb_cache.insert(bb.declist_name.clone(), (m, q, h));
                        }
                    }
                }
            }

            progress.sprite_cache = Some(cache);
            progress.billboard_cache = Some(bb_cache);
            progress.step = progress.step.next();
        }
        LoadingStep::Done => {
            // Move all prepared data into PreparedWorld resource
            let odm = progress.odm.take();
            let terrain_mesh = progress.terrain_mesh.take();
            let terrain_texture = progress.terrain_texture.take();
            let models = progress.models.take();

            if let (Some(map), Some(mesh), Some(texture), Some(models)) =
                (odm, terrain_mesh, terrain_texture, models)
            {
                let water_cells = progress.water_cells.take().unwrap_or_default();
                let water_texture = progress.water_texture.take();
                let billboards = progress.billboards.take().unwrap_or_default();
                let actors = progress.actors.take().unwrap_or_default();
                commands.insert_resource(PreparedWorld {
                    map,
                    terrain_mesh: mesh,
                    terrain_texture: texture,
                    water_texture,
                    water_cells,
                    models,
                    billboards,
                    actors,
                    monsters: progress.monsters.take().unwrap_or_default(),
                    start_points: progress.start_points.take().unwrap_or_default(),
                    sprite_cache: progress.sprite_cache.take().unwrap_or_default(),
                    billboard_cache: progress.billboard_cache.take().unwrap_or_default(),
                    music_track: progress.music_track,
                });
                commands.remove_resource::<LoadingProgress>();
                game_state.set(GameState::Game);
            }
        }
    }
}

