//! Sprite material extension and hand-written `AsBindGroup` impl.
//!
//! Every billboard sprite in the game (decorations, NPCs, monsters) uses an
//! `ExtendedMaterial<StandardMaterial, SpriteExtension>`. The extension owns
//! no per-material tint data — instead it pulls a shared GPU uniform buffer
//! out of [`SpriteGlobalsBuffer`] in its `AsBindGroup` impl, so every
//! material's bind group points at the same wgpu `Buffer` and the shader
//! reads the current day/night tint through one global path.
//!
//! The single per-material piece is `selflit: bool`, which selects between
//! the `regular` and `selflit` vec4s inside `SpriteGlobals`. It's baked into
//! the pipeline via `AsBindGroup::Data = u32` and surfaced to WGSL as a
//! shader def (`SPRITE_SELFLIT`), so selection happens at shader
//! specialization time — two pipeline variants total, zero per-material GPU
//! memory.
//!
//! **Why hand-written `AsBindGroup`?** The derive macro only knows how to
//! wire per-material fields into a bind group. It has no syntax for "pull
//! this binding out of a render-world resource." We need exactly that
//! behaviour — the buffer lives in `SpriteGlobalsBuffer` (render world) and
//! every material should reference the same `Buffer` — so the impl below
//! sets `type Param = SRes<SpriteGlobalsBuffer>` and pulls the buffer from
//! the param inside `unprepared_bind_group`. Bevy only calls
//! `as_bind_group` when a material is created or changed, and we never
//! mark sprite materials dirty for tint updates, so each bind group is built
//! exactly once and keeps a stable reference to the shared buffer for its
//! entire lifetime. `queue.write_buffer` (in
//! `sprites::tint_buffer::write_sprite_globals`) updates the contents in
//! place without touching any bind group.
//!
//! **Why not `ShaderStorageBuffer` via a `Handle`?** Tried that in commit
//! `0676f4f` (A1). `GpuShaderStorageBuffer::prepare_asset` reallocates the
//! wgpu `Buffer` on every `set_data` call, and material bind groups still
//! reference the old one — there's no invalidation path. The observable
//! symptom was sprite tints frozen at whatever value was current when each
//! material was first prepared. See `docs/todo.md` "Root cause of the A1
//! sprite tint regression" for the full trace.

use bevy::ecs::system::{SystemParamItem, lifetimeless::SRes};
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, AsBindGroupError, BindGroupLayout, BindGroupLayoutEntry, BindingResources, BindingType,
    BufferBindingType, OwnedBindingResource, RenderPipelineDescriptor, ShaderStages, ShaderType,
    SpecializedMeshPipelineError, UnpreparedBindGroup,
};
use bevy::render::renderer::RenderDevice;
use bevy::shader::{ShaderDefVal, ShaderRef};

use crate::game::sprites::tint_buffer::{GpuSpriteGlobals, SpriteGlobalsBuffer};

/// Material type for all MM6 sprite billboards (decorations, NPCs, monsters).
pub type SpriteMaterial = ExtendedMaterial<StandardMaterial, SpriteExtension>;

/// Binding index (within the material bind group) where the shared
/// `SpriteGlobals` uniform is attached. `StandardMaterial` uses bindings
/// 0..~20; 100 is well clear of that range.
const SPRITE_GLOBALS_BINDING: u32 = 100;

/// Per-material marker for whether this sprite should read the `selflit`
/// tint (torches, campfires, braziers) or the regular day/night tint. No GPU
/// data — the flag is surfaced to the pipeline via [`AsBindGroup::Data`] as a
/// `u32` and turned into the shader def `SPRITE_SELFLIT` in
/// [`MaterialExtension::specialize`].
#[derive(Asset, Reflect, Debug, Clone, Default)]
pub struct SpriteExtension {
    pub selflit: bool,
}

impl AsBindGroup for SpriteExtension {
    /// `u32` (0 or 1) drives shader specialization. Must be `PartialEq + Eq
    /// + Hash + Clone + Copy` for Bevy's material pipeline cache to key on
    /// it.
    type Data = u32;
    /// Pull the shared GPU buffer out of the render-world resource. Because
    /// this is only read here (not written), `SRes` is enough.
    type Param = SRes<SpriteGlobalsBuffer>;

    fn label() -> &'static str {
        "sprite_extension"
    }

    fn bind_group_data(&self) -> Self::Data {
        self.selflit as u32
    }

    fn unprepared_bind_group(
        &self,
        _layout: &BindGroupLayout,
        _render_device: &RenderDevice,
        globals: &mut SystemParamItem<'_, '_, Self::Param>,
        _force_no_bindless: bool,
    ) -> Result<UnpreparedBindGroup, AsBindGroupError> {
        // `init_sprite_globals` (sprites::tint_buffer) runs in `RenderStartup`
        // and guarantees the buffer exists before any material asset prepare
        // pass. If it's somehow missing (e.g. headless test without the
        // render sub-app), bail out so the caller can retry next frame.
        let Some(buffer) = globals.buffer.buffer() else {
            return Err(AsBindGroupError::RetryNextUpdate);
        };
        Ok(UnpreparedBindGroup {
            bindings: BindingResources(vec![(
                SPRITE_GLOBALS_BINDING,
                OwnedBindingResource::Buffer(buffer.clone()),
            )]),
        })
    }

    fn bind_group_layout_entries(_render_device: &RenderDevice, _force_no_bindless: bool) -> Vec<BindGroupLayoutEntry> {
        vec![BindGroupLayoutEntry {
            binding: SPRITE_GLOBALS_BINDING,
            visibility: ShaderStages::VERTEX_FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: Some(GpuSpriteGlobals::min_size()),
            },
            count: None,
        }]
    }
}

impl MaterialExtension for SpriteExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/sprite_tint.wgsl".into()
    }

    /// Add the `SPRITE_SELFLIT` shader def for materials whose `selflit`
    /// flag was true. The shader uses `#ifdef SPRITE_SELFLIT` to pick
    /// `sprite_globals.selflit` instead of `sprite_globals.regular`. Two
    /// pipeline variants total — cheap and static.
    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if key.bind_group_data == 1
            && let Some(fragment) = descriptor.fragment.as_mut()
        {
            fragment.shader_defs.push(ShaderDefVal::from("SPRITE_SELFLIT"));
        }
        Ok(())
    }
}

/// Build the standard unlit billboard sprite material used by every
/// decoration, NPC and monster billboard. All billboards share the same PBR
/// settings (unlit, alpha-masked, two-sided, no roughness/reflectance); only
/// the texture and the `selflit` kind selector vary.
pub fn unlit_billboard_material(texture: Handle<Image>, selflit: bool) -> SpriteMaterial {
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
        extension: SpriteExtension { selflit },
    }
}
