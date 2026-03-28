use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSamplerDescriptor},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

use std::collections::HashMap;

use crate::{
    assets::GameAssets,
    despawn_all,
    game::odm::OdmName,
    GameState,
};
use lod::{
    billboard::BillboardManager,
    ddm::{Ddm, DdmActor},
    dtile::{Dtile, TileTable},
    mapstats::MapStats,
    monlist::MonsterList,
    odm::{Odm, OdmData, SpawnPoint},
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
    pub map_name: OdmName,
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
    water_cells: Option<Vec<bool>>,
}

pub struct PreparedModel {
    /// Sub-meshes, one per unique texture in the BSP model.
    pub sub_meshes: Vec<PreparedSubMesh>,
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
}

pub struct PreparedBillboard {
    /// Position in Bevy coordinates.
    pub position: Vec3,
    /// Size in game units (width, height).
    pub width: f32,
    pub height: f32,
    /// Sprite image.
    pub image: Image,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum LoadingStep {
    #[default]
    ParseMap,
    BuildTerrain,
    BuildAtlas,
    BuildModels,
    BuildBillboards,
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
            Self::Done => "Done!",
        }
    }

    fn next(&self) -> Self {
        match self {
            Self::ParseMap => Self::BuildTerrain,
            Self::BuildTerrain => Self::BuildAtlas,
            Self::BuildAtlas => Self::BuildModels,
            Self::BuildModels => Self::BuildBillboards,
            Self::BuildBillboards => Self::Done,
            Self::Done => Self::Done,
        }
    }
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
    pub water_cells: Vec<bool>,
}

#[derive(Component)]
struct LoadingText;

fn loading_setup(
    mut commands: Commands,
    load_request: Option<Res<LoadRequest>>,
    save_data: Res<crate::save::GameSave>,
) {
    let map_name = load_request
        .map(|r| r.map_name.clone())
        .unwrap_or_else(|| OdmName {
            x: save_data.map.map_x,
            y: save_data.map.map_y,
        });

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
        water_cells: None,
    });

    // Keep the load request around as context
    commands.insert_resource(LoadRequest { map_name });

    // Spawn loading screen UI
    commands.spawn((Camera2d, InLoading));
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.05, 0.05, 0.05)),
            InLoading,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Loading..."),
                TextFont {
                    font_size: 40.0,
                    ..default()
                },
                TextColor(Color::WHITE),
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
) {
    // Update loading text
    for mut text in &mut text_query {
        **text = progress.step.label().to_string();
    }

    match progress.step {
        LoadingStep::ParseMap => {
            let map_name = load_request.map_name.to_string();
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

                            // Resolve spawn points to monsters via mapstats + dmonlist
                            let mut monsters = Vec::new();
                            info!("Spawn points: {}, map_name: '{}'", odm.spawn_points.len(), map_name);
                            if let (Ok(mapstats), Ok(monlist)) = (
                                MapStats::new(game_assets.lod_manager()),
                                MonsterList::new(game_assets.lod_manager()),
                            ) {
                                info!("MapStats has {} maps, looking for '{}'", mapstats.maps.len(), map_name);
                                if let Some(map_config) = mapstats.get(&map_name) {
                                    info!("Map monsters: {:?}", map_config.monster_names);
                                    for sp in &odm.spawn_points {
                                        if let Some((mon_name, dif)) = map_config.monster_for_index(sp.monster_index) {
                                            if let Some(desc) = monlist.find_with_sprite(mon_name, dif, game_assets.lod_manager()) {
                                                // Each spawn point creates a group of 3-5 monsters
                                                let group_size = 3 + ((sp.position[0].unsigned_abs() + sp.position[1].unsigned_abs()) % 3) as i32;
                                                for g in 0..group_size {
                                                    // Spread monsters around the spawn point
                                                    let angle = g as f32 * 2.094; // ~120° apart
                                                    let spread = sp.radius.max(200) as f32 * 0.5;
                                                    let offset_x = (angle.cos() * spread * g as f32) as i32;
                                                    let offset_y = (angle.sin() * spread * g as f32) as i32;
                                                    monsters.push(PreparedMonster {
                                                        position: [
                                                            sp.position[0] + offset_x,
                                                            sp.position[1] + offset_y,
                                                            sp.position[2],
                                                        ],
                                                        radius: sp.radius.max(300),
                                                        standing_sprite: desc.sprite_names[0].clone(),
                                                        walking_sprite: desc.sprite_names[1].clone(),
                                                        height: desc.height,
                                                        move_speed: desc.move_speed,
                                                        hostile: true,
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            progress.monsters = Some(monsters);

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
                        let image = Image::from_dynamic(
                            atlas,
                            true,
                            RenderAssetUsages::RENDER_WORLD,
                        );
                        progress.terrain_texture = Some(image);

                        // Load water texture
                        progress.water_texture = game_assets
                            .lod_manager()
                            .bitmap("wtrtyl")
                            .map(|img| {
                                let mut water_img = Image::from_dynamic(
                                    img,
                                    true,
                                    RenderAssetUsages::RENDER_WORLD,
                                );
                                water_img.sampler = bevy::image::ImageSampler::Descriptor(
                                    bevy::image::ImageSamplerDescriptor {
                                        address_mode_u: bevy::image::ImageAddressMode::Repeat,
                                        address_mode_v: bevy::image::ImageAddressMode::Repeat,
                                        ..default()
                                    },
                                );
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
                                        let mut image = Image::from_dynamic(
                                            img,
                                            true,
                                            RenderAssetUsages::RENDER_WORLD,
                                        );
                                        image.sampler = bevy::image::ImageSampler::Descriptor(
                                            ImageSamplerDescriptor {
                                                address_mode_u: ImageAddressMode::Repeat,
                                                address_mode_v: ImageAddressMode::Repeat,
                                                ..default()
                                            },
                                        );
                                        image
                                    });

                                let material = StandardMaterial {
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
                        PreparedModel { sub_meshes }
                    })
                    .collect();
                progress.models = Some(models);
                progress.step = progress.step.next();
            }
        }
        LoadingStep::BuildBillboards => {
            if let Some(odm) = &progress.odm {
                let mut prepared_billboards = Vec::new();
                if let Ok(bb_mgr) = BillboardManager::new(game_assets.lod_manager()) {
                    for bb in &odm.billboards {
                        if bb.data.is_invisible() {
                            continue;
                        }
                        if let Some(sprite) = bb_mgr.get(
                            game_assets.lod_manager(),
                            &bb.declist_name,
                            bb.data.declist_id,
                        ) {
                            let (w, h) = sprite.dimensions();
                            // MM6 coords (x, y, z) → Bevy (x, z, -y)
                            let pos = Vec3::new(
                                bb.data.position[0] as f32,
                                bb.data.position[2] as f32,
                                -bb.data.position[1] as f32,
                            );
                            let image = Image::from_dynamic(
                                sprite.image,
                                true,
                                RenderAssetUsages::RENDER_WORLD,
                            );
                            prepared_billboards.push(PreparedBillboard {
                                position: pos,
                                width: w,
                                height: h,
                                image,
                            });
                        }
                    }
                }
                progress.billboards = Some(prepared_billboards);
                progress.step = progress.step.next();
            }
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
                });
                commands.remove_resource::<LoadingProgress>();
                game_state.set(GameState::Game);
            }
        }
    }
}

