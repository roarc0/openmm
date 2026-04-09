//! Current day/night tint values for sprite materials.
//!
//! Used at sprite creation time (at spawn or via runtime swaps like
//! `SetSprite`) so brand-new materials are built with the current tint and
//! don't flash from `Vec4::ONE` to the real tint on the next crossing.
//!
//! **History.** This module briefly held two `ShaderStorageBuffer` handles
//! (commit `0676f4f` / A1) that sprite materials all referenced, so a single
//! in-place `set_data` on the buffer would update every sprite. That design
//! is broken on Bevy 0.18: `prepare_asset` on `GpuShaderStorageBuffer`
//! creates a new wgpu `Buffer` every time, and Bevy does not invalidate
//! materials whose bind groups reference the old buffer. The symptom was
//! sprite tints frozen at whatever value happened to be current when each
//! material was first prepared.
//!
//! We are back to the pre-A1 design: per-material `#[uniform(100)] tint`,
//! pushed by `lighting::animate_day_cycle` iterating sprite materials on
//! threshold crossings. This resource is the small piece A1 got right —
//! fresh materials created at runtime need to know the current tint so they
//! don't appear full-bright until the next crossing.
//!
//! The name `SpriteTintBuffers` is kept for call-site compatibility even
//! though there are no GPU buffers involved anymore.
//!
//! See [`SpriteExtension`](super::material::SpriteExtension) for the full
//! reasoning.

use bevy::prelude::*;

/// Current tint values for sprite materials, used at creation time.
///
/// `regular` is the ambient day/night tint that most billboards use;
/// `selflit` is a much lighter tint applied to torches, campfires, braziers,
/// and other light sources so they feel grounded in the scene without being
/// dimmed when the sun goes down.
///
/// Initial value is `Vec4::ONE` on both fields. `animate_day_cycle`
/// overwrites them on the first frame of `GameState::Game`.
#[derive(Resource, Clone, Copy, Debug)]
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

/// Registers [`SpriteTintBuffers`] as a resource at app startup so every
/// spawn site can pull the current tint without plumbing it through a
/// separate parameter.
pub struct SpriteTintBufferPlugin;

impl Plugin for SpriteTintBufferPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpriteTintBuffers>();
    }
}
