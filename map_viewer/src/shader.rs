use bevy::asset::HandleId;
use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::render::render_resource::{AsBindGroup, ShaderRef, ShaderType};
use bevy::utils::HashMap;

pub struct ShaderPlugin;

impl Plugin for ShaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(MaterialPlugin::<RepeatedMaterial>::default())
            .add_systems(Schedule::Start, setup_shader);
    }
}

#[derive(Resource, Debug, Clone)]
pub struct Materials {
    /// (Texture asset ID, Repeats) -> RepeatedMaterial
    pub repeated: HashMap<(HandleId, Repeats), Handle<RepeatedMaterial>>,
}

fn setup_shader(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(Materials {
        repeated: HashMap::new(),
    });
}

#[derive(Clone, Copy, ShaderType, Debug, Hash, Eq, PartialEq)]
pub struct Repeats {
    pub horizontal: u32,
    pub vertical: u32,
}

#[derive(AsBindGroup, Debug, Clone, TypeUuid)]
#[uuid = "82d336c5-fd6c-41a3-bdd4-267cd4c9be22"]
pub struct RepeatedMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub texture: Option<Handle<Image>>,
    #[uniform(2)]
    pub repeats: Repeats,
}

impl Material for RepeatedMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/repeated.wgsl".into()
    }

    fn vertex_shader() -> ShaderRef {
        ShaderRef::Default
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }

    fn depth_bias(&self) -> f32 {
        0.0
    }

    fn prepass_vertex_shader() -> ShaderRef {
        ShaderRef::Default
    }

    fn prepass_fragment_shader() -> ShaderRef {
        ShaderRef::Default
    }

    fn specialize(
        pipeline: &bevy::pbr::MaterialPipeline<Self>,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        layout: &bevy::render::mesh::MeshVertexBufferLayout,
        key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        Ok(())
    }
}
