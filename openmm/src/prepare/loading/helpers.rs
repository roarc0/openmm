//! Shared mesh and material builders used by both indoor and outdoor loading.
use bevy::{asset::RenderAssetUsages, mesh::PrimitiveTopology, prelude::*};

use super::texture_emissive;

/// Build a render-only triangle mesh from positions, normals, and UVs.
pub(super) fn build_textured_mesh(positions: Vec<[f32; 3]>, normals: Vec<[f32; 3]>, uvs: Vec<[f32; 2]>) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    _ = mesh.generate_tangents();
    mesh
}

/// Iterate a list of texture names and collect their (width, height) from the LOD.
pub(super) fn collect_texture_sizes<'a>(
    names: impl IntoIterator<Item = &'a String>,
    game_assets: &crate::assets::GameAssets,
) -> std::collections::HashMap<String, (u32, u32)> {
    let mut texture_sizes = std::collections::HashMap::new();
    for name in names {
        if name.is_empty() || texture_sizes.contains_key(name) {
            continue;
        }
        if let Some(img) = game_assets.lod().bitmap(name) {
            texture_sizes.insert(name.clone(), (img.width(), img.height()));
        }
    }
    texture_sizes
}

/// Standard material for indoor BLV surfaces — matte stone/brick, no sun boost.
pub(super) fn indoor_material(texture_name: &str) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::WHITE,
        alpha_mode: AlphaMode::Opaque,
        // MM6 BSP uses CW winding; from inside the room these are
        // back faces in Bevy's CCW convention. cull_mode:None renders
        // them, and double_sided MUST be false so normals are not
        // flipped — the stored normals already point into the room,
        // which is correct for PBR lighting from interior lights.
        cull_mode: None,
        double_sided: false,
        perceptual_roughness: 0.85,
        reflectance: 0.2,
        metallic: 0.0,
        emissive: texture_emissive(texture_name),
        ..default()
    }
}

/// Standard material for outdoor ODM BSP models — sun-boosted base color
/// with warm specular tint for sunlight atmosphere.
pub(super) fn outdoor_material(texture_name: &str) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgb(1.8, 1.8, 1.8),
        alpha_mode: AlphaMode::Opaque,
        cull_mode: None,
        double_sided: false,
        perceptual_roughness: 0.85,
        reflectance: 0.2,
        specular_tint: Color::srgb(1.0, 0.95, 0.85),
        metallic: 0.0,
        emissive: texture_emissive(texture_name),
        ..default()
    }
}
