//! Runtime texture replacement for outdoor BSP faces, driven by EVT events.

use bevy::ecs::message::{Message, MessageReader};
use bevy::prelude::*;

use crate::assets::GameAssets;

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

/// Handle `ApplyTextureOutdoors` messages — swap the material on the matching BSP sub-mesh entity.
pub(super) fn apply_texture_outdoors(
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

        let Some(img) = game_assets.lod().bitmap(&ev.texture_name) else {
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
