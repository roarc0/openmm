use bevy::{
    prelude::*,
    pbr::{ExtendedMaterial, MaterialExtension},
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
}

impl MaterialExtension for WaterExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/terrain_water.wgsl".into()
    }
}

/// Update terrain material is a no-op — water animation uses globals.time
/// directly in the shader.
pub fn update_terrain_time(
    _time: Res<Time>,
    _materials: ResMut<Assets<TerrainMaterial>>,
) {
}
