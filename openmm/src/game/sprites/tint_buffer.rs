//! Globally-shared sprite tint uniform buffer.
//!
//! One `UniformBuffer<GpuSpriteGlobals>` (created in `RenderStartup`, owned
//! by a render-world resource) holds two `vec4<f32>` values — `regular` and
//! `selflit` — that every sprite material reads via its bind group. The
//! buffer is updated once per frame in `Render::PrepareResources`, and
//! because Bevy's [`UniformBuffer`] only reallocates when its data layout
//! changes (which never happens for us after the first write), subsequent
//! frames just call `queue.write_buffer` under the hood. The wgpu `Buffer`
//! handle stays stable → no bind-group invalidation → every sprite sees
//! the updated tint on the next frame.
//!
//! **Why this exists.** Previous approaches pushed tint updates per-material:
//!
//!   - The pre-A1 design wrote `mat.extension.tint` on every sprite material
//!     on threshold crossings. A dense outdoor map has ~20k sprite materials,
//!     and touching all of them in one frame hitched perceptibly.
//!   - A1 (commit `0676f4f`) tried to route tint through a shared
//!     `Handle<ShaderStorageBuffer>` on each material, but
//!     `GpuShaderStorageBuffer::prepare_asset` creates a brand-new wgpu
//!     `Buffer` on every `Assets::set_data` call, and Bevy does not invalidate
//!     material bind groups that still reference the old buffer — so tints
//!     froze at whatever value was current when each material was first
//!     prepared.
//!
//! This module takes the third path: bypass Bevy's asset system entirely for
//! the tint buffer. `SpriteExtension`'s hand-written [`AsBindGroup`] impl
//! pulls the `Buffer` straight out of [`SpriteGlobalsBuffer`] via its
//! `Param = SRes<SpriteGlobalsBuffer>`. Since `as_bind_group` only runs on
//! material creation (not per frame), each material keeps a stable binding
//! to our one owned buffer for its entire lifetime.
//!
//! **Per-material selflit selector.** Torches and campfires want a lighter
//! tint than the general day/night ambient. The selector is baked into each
//! material at spawn time as `SpriteExtension::selflit: bool`, surfaced to
//! the pipeline via `AsBindGroup::Data = u32`, and turned into the shader def
//! `SPRITE_SELFLIT` by `MaterialExtension::specialize`. The shader picks
//! `sprite_globals.selflit` or `sprite_globals.regular` via `#ifdef`, so
//! there's one pipeline variant per kind and zero per-material GPU memory
//! overhead beyond the shared buffer.

use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_resource::{ShaderType, UniformBuffer};
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};

/// Main-world resource holding the current day/night sprite tints in linear
/// sRGB. Written by `lighting::animate_day_cycle` every frame; extracted into
/// the render world by [`ExtractResourcePlugin`] and uploaded to the GPU by
/// [`write_sprite_globals`].
///
/// `regular` is the ambient day/night tint that most billboards use;
/// `selflit` is a much lighter tint applied to torches, campfires, braziers,
/// and other light sources so they feel grounded in the scene without being
/// dimmed when the sun goes down.
#[derive(Resource, ExtractResource, Clone, Copy, Debug)]
pub struct SpriteTintBuffers {
    pub regular: Vec4,
    pub selflit: Vec4,
}

impl Default for SpriteTintBuffers {
    fn default() -> Self {
        Self {
            regular: Vec4::ONE,
            selflit: Vec4::ONE,
        }
    }
}

/// GPU representation of the two tints. Layout matches the WGSL
/// `SpriteGlobals` struct in `sprite_tint.wgsl` — two `vec4<f32>` fields, no
/// padding required (16-byte aligned naturally).
#[derive(Clone, Copy, ShaderType, Default)]
pub struct GpuSpriteGlobals {
    pub regular: Vec4,
    pub selflit: Vec4,
}

/// Render-world resource wrapping the one `UniformBuffer` that every sprite
/// material's bind group points at. Created in [`init_sprite_globals`] with
/// an initial `write_buffer` call so the GPU-side buffer exists before any
/// material asset preparation runs. Subsequent per-frame writes from
/// [`write_sprite_globals`] hit the in-place `queue.write_buffer` path,
/// leaving the wgpu `Buffer` handle stable.
#[derive(Resource, Default)]
pub struct SpriteGlobalsBuffer {
    pub buffer: UniformBuffer<GpuSpriteGlobals>,
}

/// Plugin: initialises the main-world [`SpriteTintBuffers`] resource, extracts
/// it into the render world, creates the shared GPU uniform buffer on render
/// startup, and writes the tint data into it once per frame in
/// `RenderSystems::PrepareResources`.
pub struct SpriteTintBufferPlugin;

impl Plugin for SpriteTintBufferPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpriteTintBuffers>()
            .add_plugins(ExtractResourcePlugin::<SpriteTintBuffers>::default());

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .init_resource::<SpriteGlobalsBuffer>()
            .add_systems(RenderStartup, init_sprite_globals)
            .add_systems(Render, write_sprite_globals.in_set(RenderSystems::PrepareResources));
    }
}

/// Allocate the backing wgpu `Buffer` inside the `UniformBuffer` once, before
/// any material prepare pass runs. We default-initialise the contents to
/// `Vec4::ONE` (no tint) — the first real tint write happens on the same
/// frame via [`write_sprite_globals`], so materials created on frame 1 will
/// already see correct values even though they prepare before the render
/// systems run.
///
/// Calling `write_buffer` while `self.buffer.is_none()` is the sanctioned way
/// in Bevy 0.18 to force an initial allocation — see
/// `bevy_render::globals::prepare_globals_buffer` for the same pattern.
fn init_sprite_globals(mut globals: ResMut<SpriteGlobalsBuffer>, device: Res<RenderDevice>, queue: Res<RenderQueue>) {
    globals.buffer.set(GpuSpriteGlobals::default());
    globals.buffer.write_buffer(&device, &queue);
}

/// Push the current main-world tint values into the shared GPU buffer. Runs
/// every frame in `RenderSystems::PrepareResources`. Because `UniformBuffer`
/// only reallocates its wgpu `Buffer` when either the buffer doesn't exist or
/// the `changed` flag is set (which we never trigger — `set()` leaves it
/// false), this hits the in-place `queue.write_buffer` path on every frame
/// after initialisation. The Buffer handle stays stable, so every sprite
/// material's existing bind group keeps pointing at the same resource with
/// updated contents.
fn write_sprite_globals(
    tints: Res<SpriteTintBuffers>,
    mut globals: ResMut<SpriteGlobalsBuffer>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    globals.buffer.set(GpuSpriteGlobals {
        regular: tints.regular,
        selflit: tints.selflit,
    });
    globals.buffer.write_buffer(&device, &queue);
}
