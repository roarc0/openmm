use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::{AsBindGroup, Face},
    shader::ShaderRef,
};

use crate::game::InGame;

/// Type alias for the full terrain material (StandardMaterial + water extension).
pub type TerrainMaterial = ExtendedMaterial<StandardMaterial, WaterExtension>;

/// Extension to StandardMaterial that replaces cyan marker pixels with
/// animated water. All PBR lighting, fog, shadows come from StandardMaterial.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct WaterExtension {
    #[texture(100)]
    #[sampler(101)]
    pub water_texture: Handle<Image>,
    /// R8 mask: white = water pixel, black = terrain. Uses nearest filtering
    /// so water boundaries stay sharp even when the terrain atlas is linear-filtered.
    #[texture(102)]
    #[sampler(103)]
    pub water_mask: Handle<Image>,
}

impl MaterialExtension for WaterExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/water.wgsl".into()
    }
}

/// Spawns the terrain entity. If `terrain_materials` assets are provided,
/// uses the specialized `TerrainMaterial`. Otherwise, falls back to `StandardMaterial`.
pub fn spawn_terrain(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    terrain_materials: &mut Option<ResMut<Assets<TerrainMaterial>>>,
    terrain_mesh: Mesh,
    terrain_tex_handle: Handle<Image>,
    water_tex_handle: Handle<Image>,
    water_mask_handle: Handle<Image>,
) -> Entity {
    let mut terrain_entity = commands.spawn((
        Name::new("odm"),
        Mesh3d(meshes.add(terrain_mesh)),
        Transform::default(),
        Visibility::default(),
        InGame,
    ));

    if let Some(tm) = terrain_materials.as_mut() {
        let terrain_mat_handle = tm.add(TerrainMaterial {
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
        terrain_entity.insert(MeshMaterial3d(terrain_mat_handle));
    } else {
        let terrain_mat_handle = materials.add(StandardMaterial {
            base_color_texture: Some(terrain_tex_handle),
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            metallic: 0.0,
            cull_mode: Some(Face::Back),
            ..default()
        });
        terrain_entity.insert(MeshMaterial3d(terrain_mat_handle));
    }

    terrain_entity.id()
}
