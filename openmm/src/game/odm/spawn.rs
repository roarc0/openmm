//! `spawn_world` orchestrates the one-time outdoor map spawn at `OnEnter(Game)`.
//!
//! Builds the terrain entity, parents BSP buildings under it, computes the
//! distance-sorted spawn order for decorations / actors / monsters, then hands
//! off to [`super::lazy_spawn`] for time-budgeted per-frame spawning.

use bevy::prelude::*;

use crate::game::optional::OptionalWrite;
use crate::game::sprite_material::SpriteMaterial;
use crate::states::loading::PreparedWorld;

use super::terrain;

use super::lazy_spawn::{PendingSpawns, SpawnProgress, sort_by_distance_mm6};
use super::texture_swap::BspSubMesh;

pub(super) fn spawn_world(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    _sprite_materials: Option<ResMut<Assets<SpriteMaterial>>>,
    mut terrain_materials: Option<ResMut<Assets<terrain::TerrainMaterial>>>,
    mut prepared: Option<ResMut<PreparedWorld>>,
    save_data: Res<crate::save::GameSave>,
    cfg: Res<crate::config::GameConfig>,
    mut music_events: Option<bevy::ecs::message::MessageWriter<crate::game::sound::music::PlayMusicEvent>>,
) {
    let Some(prepared) = prepared.as_mut() else {
        // No outdoor PreparedWorld — this is an indoor map, skip outdoor spawning
        return;
    };

    let (terrain_tex_handle, water_tex_handle, water_mask_handle) =
        prepare_terrain_textures(prepared, &mut images, &cfg);

    let terrain_entity_id = terrain::spawn_terrain(
        &mut commands,
        &mut meshes,
        &mut materials,
        terrain_materials.as_deref_mut(),
        prepared.terrain_mesh.clone(),
        terrain_tex_handle,
        water_tex_handle,
        water_mask_handle,
    );

    spawn_bsp_models(
        &mut commands,
        terrain_entity_id,
        prepared,
        &mut meshes,
        &mut materials,
        &mut images,
        &cfg,
    );

    let player_spawn = Vec3::new(
        save_data.player.position[0],
        save_data.player.position[1],
        save_data.player.position[2],
    );
    let orders = compute_spawn_orders(prepared, player_spawn);

    music_events.try_write(crate::game::sound::music::PlayMusicEvent {
        track: prepared.music_track,
        volume: cfg.music_volume,
    });

    let total = orders.billboard.len() + orders.actor.len() + orders.monster.len();
    commands.insert_resource(SpawnProgress { total, done: 0 });
    commands.insert_resource(PendingSpawns {
        billboard_order: orders.billboard,
        actor_order: orders.actor,
        idx: 0,
        frames_elapsed: 0,
        sprite_cache: prepared.sprite_cache.clone(),
        dec_sprite_cache: prepared.dec_sprite_cache.clone(),
        decorations: prepared.decorations.clone(),
        actors: prepared.resolved_actors.take(),
        monsters: prepared.resolved_monsters.take(),
        monster_order: orders.monster,
        terrain_entity: terrain_entity_id,
    });

    spawn_outdoor_clickable_faces(&mut commands, prepared);
}

/// Distance-sorted spawn orders for the three entity kinds.
struct SpawnOrders {
    billboard: Vec<usize>,
    actor: Vec<usize>,
    monster: Vec<usize>,
}

fn compute_spawn_orders(prepared: &PreparedWorld, player_spawn: Vec3) -> SpawnOrders {
    let billboard = sort_by_distance_mm6(
        prepared.decorations.entries(),
        player_spawn,
        |d| d.position[0] as f32,
        |d| d.position[1] as f32,
    );

    let actor = prepared
        .resolved_actors
        .as_ref()
        .map(|a| {
            sort_by_distance_mm6(
                a.get_actors(),
                player_spawn,
                |actor| actor.position[0] as f32,
                |actor| actor.position[1] as f32,
            )
        })
        .unwrap_or_default();

    let monster = prepared
        .resolved_monsters
        .as_ref()
        .map(|m| {
            sort_by_distance_mm6(
                m.entries(),
                player_spawn,
                |mon| mon.spawn_position[0] as f32,
                |mon| mon.spawn_position[1] as f32,
            )
        })
        .unwrap_or_default();

    SpawnOrders {
        billboard,
        actor,
        monster,
    }
}

/// Builds clone'd terrain/water/water-mask images with the right samplers and uploads them.
fn prepare_terrain_textures(
    prepared: &PreparedWorld,
    images: &mut Assets<Image>,
    cfg: &crate::config::GameConfig,
) -> (Handle<Image>, Handle<Image>, Handle<Image>) {
    let mut terrain_texture = prepared.terrain_texture.clone();
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

    (terrain_tex_handle, water_tex_handle, water_mask_handle)
}

/// Spawn BSP buildings as children of the terrain entity.
fn spawn_bsp_models(
    commands: &mut Commands,
    terrain_entity_id: Entity,
    prepared: &PreparedWorld,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    cfg: &crate::config::GameConfig,
) {
    let model_sampler = crate::assets::sampler_for_filtering(&cfg.models_filtering);
    commands.entity(terrain_entity_id).with_children(|parent| {
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
    });
}

/// Build outdoor clickable + occluder faces from BSP model geometry.
fn spawn_outdoor_clickable_faces(commands: &mut Commands, prepared: &PreparedWorld) {
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
            let normal = Vec3::from(openmm_data::odm::mm6_normal_to_bevy(face.plane.normal));
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
