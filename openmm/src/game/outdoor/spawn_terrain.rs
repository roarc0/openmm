use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::{AsBindGroup, Face},
    shader::ShaderRef,
};

use crate::{game::InGame, prepare::loading::PreparedWorld};

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

/// Spawns the terrain entity. If `terrain_materials` is provided, uses the
/// specialized `TerrainMaterial` (water shader). Otherwise falls back to a
/// plain `StandardMaterial` so terrain still renders when the
/// `TerrainMaterial` plugin is disabled.
pub fn spawn_terrain(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    terrain_materials: Option<&mut Assets<TerrainMaterial>>,
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

    if let Some(tm) = terrain_materials {
        let terrain_mat_handle = tm.add(TerrainMaterial {
            base: StandardMaterial {
                base_color_texture: Some(terrain_tex_handle),
                perceptual_roughness: 0.95,
                reflectance: 0.1,
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
            perceptual_roughness: 0.95,
            reflectance: 0.1,
            metallic: 0.0,
            cull_mode: Some(Face::Back),
            ..default()
        });
        terrain_entity.insert(MeshMaterial3d(terrain_mat_handle));
    }

    terrain_entity.id()
}

/// Builds clone'd terrain/water/water-mask images with the right samplers and uploads them.
pub(super) fn prepare_terrain_textures(
    prepared: &PreparedWorld,
    images: &mut Assets<Image>,
    cfg: &crate::system::config::GameConfig,
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
