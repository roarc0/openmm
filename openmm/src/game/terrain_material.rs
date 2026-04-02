use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

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
        "shaders/terrain_water.wgsl".into()
    }
}
