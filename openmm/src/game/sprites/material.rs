use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::{render_resource::AsBindGroup, storage::ShaderStorageBuffer},
    shader::ShaderRef,
};

/// Material type for all MM6 sprite billboards (decorations, NPCs, monsters).
/// Extends StandardMaterial with a shared day/night tint storage buffer bound
/// in the fragment shader.
pub type SpriteMaterial = ExtendedMaterial<StandardMaterial, SpriteExtension>;

/// Extension data for sprite materials.
///
/// All sprite materials share one of two globally-owned `ShaderStorageBuffer`
/// handles from [`crate::game::sprites::tint_buffer::SpriteTintBuffers`]:
/// a regular tint buffer for normal billboards, and a selflit tint buffer for
/// light sources (torches, campfires). The lighting system updates the buffer
/// data in place; no per-material mutation is needed when the tint changes.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct SpriteExtension {
    /// Shared storage buffer holding a single `vec4<f32>` tint. Every sprite
    /// material holds a clone of one of the two handles from `SpriteTintBuffers`.
    #[storage(100, read_only)]
    pub tint_buffer: Handle<ShaderStorageBuffer>,
}

impl MaterialExtension for SpriteExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/sprite_tint.wgsl".into()
    }
}

/// Build the standard unlit billboard sprite material used by every decoration,
/// NPC and monster billboard. All billboards share the same PBR settings
/// (unlit, alpha-masked, two-sided, no roughness/reflectance) — only the
/// texture and which tint buffer handle they reference vary.
pub fn unlit_billboard_material(texture: Handle<Image>, tint_buffer: Handle<ShaderStorageBuffer>) -> SpriteMaterial {
    SpriteMaterial {
        base: StandardMaterial {
            unlit: true,
            base_color_texture: Some(texture),
            alpha_mode: AlphaMode::Mask(0.5),
            cull_mode: None,
            double_sided: true,
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            ..default()
        },
        extension: SpriteExtension { tint_buffer },
    }
}
