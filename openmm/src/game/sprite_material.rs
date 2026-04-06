use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

/// Material type for all MM6 sprite billboards (decorations, NPCs, monsters).
/// Extends StandardMaterial with a per-sprite tint uniform applied in the fragment shader.
pub type SpriteMaterial = ExtendedMaterial<StandardMaterial, SpriteExtension>;

/// Extension data for sprite materials — carries the day/night tint uniform.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct SpriteExtension {
    /// Day/night tint (linear sRGB). Vec4::ONE = no change.
    /// Updated each lighting tick by `animate_day_cycle`. New materials default to white
    /// and pick up the correct tint on the next threshold crossing (or immediately via
    /// `CurrentSpriteTint` when spawned by runtime events like SetSprite).
    #[uniform(100)]
    pub tint: Vec4,
}

impl Default for SpriteExtension {
    fn default() -> Self {
        Self { tint: Vec4::ONE }
    }
}

impl MaterialExtension for SpriteExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/sprite_tint.wgsl".into()
    }
}
