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
    dtile::TileTable,
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
    models: Option<Vec<PreparedModel>>,
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

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum LoadingStep {
    #[default]
    ParseMap,
    BuildTerrain,
    BuildAtlas,
    BuildModels,
    Done,
}

impl LoadingStep {
    fn label(&self) -> &'static str {
        match self {
            Self::ParseMap => "Parsing map...",
            Self::BuildTerrain => "Building terrain...",
            Self::BuildAtlas => "Building textures...",
            Self::BuildModels => "Building models...",
            Self::Done => "Done!",
        }
    }

    fn next(&self) -> Self {
        match self {
            Self::ParseMap => Self::BuildTerrain,
            Self::BuildTerrain => Self::BuildAtlas,
            Self::BuildModels => Self::Done,
            Self::BuildAtlas => Self::BuildModels,
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
    pub models: Vec<PreparedModel>,
}

#[derive(Component)]
struct LoadingText;

fn loading_setup(mut commands: Commands, load_request: Option<Res<LoadRequest>>) {
    let map_name = load_request
        .map(|r| r.map_name.clone())
        .unwrap_or_default();

    commands.insert_resource(LoadingProgress {
        step: LoadingStep::ParseMap,
        odm: None,
        tile_table: None,
        odm_data: None,
        terrain_mesh: None,
        terrain_texture: None,
        models: None,
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
        LoadingStep::Done => {
            // Move all prepared data into PreparedWorld resource
            let odm = progress.odm.take();
            let terrain_mesh = progress.terrain_mesh.take();
            let terrain_texture = progress.terrain_texture.take();
            let models = progress.models.take();

            if let (Some(map), Some(mesh), Some(texture), Some(models)) =
                (odm, terrain_mesh, terrain_texture, models)
            {
                commands.insert_resource(PreparedWorld {
                    map,
                    terrain_mesh: mesh,
                    terrain_texture: texture,
                    models,
                });
                commands.remove_resource::<LoadingProgress>();
                game_state.set(GameState::Game);
            }
        }
    }
}

