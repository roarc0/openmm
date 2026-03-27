use bevy::{
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
};

/// Custom terrain material with animated water support.
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct TerrainMaterial {
    #[uniform(0)]
    pub params: TerrainParams,
    #[texture(1)]
    #[sampler(2)]
    pub terrain_texture: Handle<Image>,
    #[texture(3)]
    #[sampler(4)]
    pub water_texture: Handle<Image>,
}

#[derive(ShaderType, Debug, Clone)]
pub struct TerrainParams {
    pub base_color: LinearRgba,
    pub time: f32,
    pub water_speed: f32,
    pub water_distortion: f32,
    pub _padding: f32,
}

impl Default for TerrainParams {
    fn default() -> Self {
        Self {
            base_color: LinearRgba::new(0.85, 0.85, 0.85, 1.0),
            time: 0.0,
            water_speed: 0.03,
            water_distortion: 0.015,
            _padding: 0.0,
        }
    }
}

impl Material for TerrainMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/terrain.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/terrain.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }
}
