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
///
/// **Why a per-material uniform instead of a shared storage buffer?**
///
/// A previous attempt (A1, commit `0676f4f`) stored the tint in a shared
/// `ShaderStorageBuffer` asset keyed by two handles (regular + selflit) and
/// updated the buffer data in place once per threshold crossing. That design
/// is elegant but fundamentally broken on Bevy 0.18: when the storage buffer
/// asset is updated, Bevy creates a brand-new wgpu `Buffer` inside
/// `prepare_asset`, and the existing material bind groups still reference the
/// **old** buffer — nothing invalidates them. The visible symptom was sprite
/// tints frozen at whatever value was current when each material was first
/// prepared, so day/night transitions never propagated to sprites and
/// differently-aged materials (preload vs runtime swap) showed different
/// stale tints.
///
/// This module is back to the pre-A1 design: every sprite material owns its
/// own `tint: Vec4`, and `lighting::animate_day_cycle` iterates all sprite
/// materials on threshold crossings to push the new value. The 20k-write
/// hitch that A1 was trying to eliminate is back, but it only happens a
/// handful of times per in-game day (amortising the iteration across frames
/// is a follow-up optimisation).
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct SpriteExtension {
    /// Day/night tint (linear sRGB, alpha = 1). `Vec4::ONE` = no change.
    /// Rewritten each threshold tick by `animate_day_cycle`; new materials
    /// created at runtime default to the current tint via `CurrentSpriteTint`.
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

/// Build the standard unlit billboard sprite material used by every decoration,
/// NPC and monster billboard. All billboards share the same PBR settings
/// (unlit, alpha-masked, two-sided, no roughness/reflectance) — only the
/// texture and per-sprite tint vary.
pub fn unlit_billboard_material(texture: Handle<Image>, tint: Vec4) -> SpriteMaterial {
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
        extension: SpriteExtension { tint },
    }
}
