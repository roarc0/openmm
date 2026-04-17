use bevy::prelude::*;

use crate::game::coords::mm6_fixed_normal_to_bevy;
use crate::prepare::loading::PreparedWorld;

use super::bsp_water::{BspWaterExtension, BspWaterMaterial, is_bsp_water_texture};
use super::texture_swap::BspSubMesh;

/// Spawn BSP buildings as children of the terrain entity.
///
/// Sub-meshes whose texture name matches [`is_bsp_water_texture`] (currently
/// `WtrTyl`) are spawned with [`BspWaterMaterial`] so they get the same
/// scroll+wave animation as terrain water; every other sub-mesh stays on
/// plain `StandardMaterial`. `bsp_water_materials` is optional so the
/// outdoor plugin still works when the water-material plugin is disabled —
/// those faces just render as static in that case.
#[allow(clippy::too_many_arguments)]
pub fn spawn_bsp_models(
    commands: &mut Commands,
    terrain_entity_id: Entity,
    prepared: &PreparedWorld,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    mut bsp_water_materials: Option<&mut Assets<BspWaterMaterial>>,
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
                    let mesh_handle = meshes.add(sub.mesh.clone());
                    let sub_marker = BspSubMesh {
                        model_index: model_index as u32,
                        face_indices: sub.face_indices.clone(),
                        texture_name: sub.texture_name.clone(),
                    };
                    if is_bsp_water_texture(&sub.texture_name)
                        && let Some(water_mats) = bsp_water_materials.as_deref_mut()
                    {
                        let water_mat = water_mats.add(BspWaterMaterial {
                            base: mat,
                            extension: BspWaterExtension::default(),
                        });
                        model_parent.spawn((Mesh3d(mesh_handle), MeshMaterial3d(water_mat), sub_marker));
                    } else {
                        model_parent.spawn((Mesh3d(mesh_handle), MeshMaterial3d(materials.add(mat)), sub_marker));
                    }
                }
            });
        }
    });
}

/// Build outdoor clickable + occluder faces from BSP model geometry.
pub fn spawn_bsp_clickable_faces(commands: &mut Commands, prepared: &PreparedWorld) {
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
            let normal = Vec3::from(mm6_fixed_normal_to_bevy(face.plane.normal));
            let plane_dist = normal.dot(verts[0]);
            if face.cog_trigger_id != 0 {
                outdoor_clickable.push(crate::game::interaction::clickable::FaceInfo {
                    face_index: 0,
                    event_id: face.cog_trigger_id,
                    normal,
                    plane_dist,
                    vertices: verts.clone(),
                });
            }
            outdoor_occluders.push(crate::game::indoor::OccluderFaceInfo {
                normal,
                plane_dist,
                vertices: verts,
            });
        }
    }
    if !outdoor_clickable.is_empty() {
        commands.insert_resource(crate::game::interaction::clickable::Faces {
            faces: outdoor_clickable,
            is_indoor: false,
        });
    }
    if !outdoor_occluders.is_empty() {
        commands.insert_resource(crate::game::indoor::OccluderFaces::new(outdoor_occluders));
    }
}
