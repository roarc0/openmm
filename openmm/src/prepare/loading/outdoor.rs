//! Outdoor (ODM) loading steps — terrain, atlas, BSP models, billboards, and finalization.
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use openmm_data::odm::OdmData;

use super::{LoadRequest, LoadingProgress, PreparedModel, PreparedSubMesh, PreparedWorld, StartPoint, helpers};
use crate::{GameState, assets::GameAssets, game::map::CurrentMap, game::map::coords::mm6_position_to_bevy};

pub(super) fn step_build_terrain(progress: &mut LoadingProgress) {
    if let (Some(odm), Some(tile_table)) = (&progress.odm, &progress.tile_table) {
        let odm_data = OdmData::new(odm, tile_table);
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
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

pub(super) fn step_build_atlas(progress: &mut LoadingProgress, game_assets: &GameAssets) {
    if let Some(tile_table) = &progress.tile_table {
        match tile_table.atlas_image(game_assets.assets()) {
            Ok(mut atlas) => {
                let mask = openmm_data::image::extract_water_mask(&mut atlas);
                progress.terrain_texture = Some(crate::assets::dynamic_to_bevy_image(atlas));
                progress.water_mask = Some(crate::assets::dynamic_to_bevy_image(mask));

                // Load water texture
                progress.water_texture = game_assets.lod().bitmap("wtrtyl").map(|img| {
                    let mut water_img = crate::assets::dynamic_to_bevy_image(img);
                    water_img.sampler = crate::assets::repeat_sampler();
                    water_img
                });

                progress.step = progress.step.next();
            }
            Err(e) => {
                error!("Failed to build atlas: {}", e);
            }
        }
    }
}

/// Build outdoor (ODM) BSP model meshes from the parsed ODM data.
pub(super) fn step_build_models_outdoor(progress: &mut LoadingProgress, game_assets: &GameAssets) {
    let odm = progress.odm.as_ref().unwrap();
    // Collect texture sizes for UV normalization
    let names = odm.bsp_models.iter().flat_map(|b| &b.texture_names);
    let texture_sizes = helpers::collect_texture_sizes(names, game_assets);

    let models = odm
        .bsp_models
        .iter()
        .map(|b| {
            let textured = b.textured_meshes(&texture_sizes);
            let sub_meshes = textured
                .into_iter()
                .map(|tm| {
                    let mesh = helpers::build_textured_mesh(tm.positions, tm.normals, tm.uvs);
                    let texture = game_assets.lod().bitmap(&tm.texture_name).map(|img| {
                        let mut image = crate::assets::dynamic_to_bevy_image(img);
                        image.sampler = crate::assets::repeat_sampler();
                        image
                    });
                    PreparedSubMesh {
                        mesh,
                        material: helpers::outdoor_material(&tm.texture_name),
                        texture,
                        texture_name: tm.texture_name.clone(),
                        face_indices: tm.face_indices.clone(),
                    }
                })
                .collect();
            let pos = mm6_position_to_bevy(b.header.position[0], b.header.position[1], b.header.position[2]);
            let mut event_ids: Vec<u16> = b
                .faces
                .iter()
                .filter_map(|f| {
                    if f.cog_trigger_id > 0 {
                        Some(f.cog_trigger_id)
                    } else {
                        None
                    }
                })
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

pub(super) fn step_build_billboards(progress: &mut LoadingProgress, game_assets: &GameAssets) {
    if progress.odm.is_some() {
        let lod = game_assets.lod();
        let mut start_points = Vec::new();

        // Extract start/teleport markers from raw billboard list (Decorations filters these out)
        // Use a scoped block to release the immutable borrow before assigning to progress.
        let decorations = {
            let odm = progress.odm.as_ref().unwrap();
            for bb in &odm.billboards {
                if bb.data.is_original_invisible() {
                    continue;
                }
                let is_marker = lod
                    .billboard_item(bb.data.declist_id)
                    .map(|item| item.is_marker() || item.is_no_draw())
                    .unwrap_or(false);
                let name_lower = bb.declist_name.to_lowercase();
                if name_lower.contains("start") || is_marker {
                    let pos = Vec3::from(mm6_position_to_bevy(
                        bb.data.position[0],
                        bb.data.position[1],
                        bb.data.position[2],
                    ));
                    let yaw = bb.data.direction_degrees as f32 * std::f32::consts::PI / 1024.0;
                    start_points.push(StartPoint {
                        name: bb.declist_name.clone(),
                        position: pos,
                        yaw,
                    });
                }
            }
            // Decorations::new filters invisible/marker/no-draw entries automatically
            openmm_data::assets::Decorations::load(game_assets.assets(), &odm.billboards).ok()
        };
        progress.start_points = Some(start_points);
        progress.decorations = decorations;
        progress.step = progress.step.next();
    }
}

pub(super) fn step_done(
    progress: &mut LoadingProgress,
    game_assets: &GameAssets,
    load_request: &LoadRequest,
    commands: &mut Commands,
    game_state: &mut NextState<GameState>,
) {
    // Move all prepared data into PreparedWorld resource
    let odm = progress.odm.take();
    let terrain_mesh = progress.terrain_mesh.take();
    let terrain_texture = progress.terrain_texture.take();
    let models = progress.models.take();

    if let (Some(map), Some(mesh), Some(texture), Some(models)) = (odm, terrain_mesh, terrain_texture, models) {
        let water_cells = progress.water_cells.take().unwrap_or_default();
        let water_texture = progress.water_texture.take();
        let map_base = match &load_request.map_name {
            openmm_data::utils::MapName::Outdoor(odm) => odm.base_name(),
            other => other.to_string(),
        };
        crate::game::events::load_map_events(commands, game_assets, &map_base, false);
        commands.insert_resource(PreparedWorld {
            map,
            terrain_mesh: mesh,
            terrain_texture: texture,
            water_mask: progress.water_mask.take(),
            water_texture,
            water_cells,
            models,
            decorations: progress
                .decorations
                .take()
                .unwrap_or_else(openmm_data::assets::Decorations::empty),
            resolved_actors: progress.resolved_actors.take(),
            resolved_monsters: progress.resolved_monsters.take(),
            start_points: progress.start_points.take().unwrap_or_default(),
            sprite_cache: progress.sprite_cache.take().unwrap_or_default(),
            dec_sprite_cache: progress.dec_sprite_cache.take().unwrap_or_default(),
            terrain_lookup: progress
                .terrain_lookup
                .take()
                .unwrap_or_else(openmm_data::terrain::TerrainLookup::empty),
            music_track: progress.music_track,
        });
        commands.insert_resource(CurrentMap(load_request.map_name.clone()));
        commands.remove_resource::<LoadingProgress>();
        commands.remove_resource::<LoadRequest>();
        game_state.set(GameState::Game);
    }
}
