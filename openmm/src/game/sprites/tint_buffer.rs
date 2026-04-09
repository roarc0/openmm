//! Shared day/night tint storage buffers for all sprite materials.
//!
//! Instead of mutating a per-material `tint: Vec4` on every sprite when the
//! day/night tint crosses a threshold, every `SpriteMaterial` references one
//! of two globally shared `ShaderStorageBuffer` assets:
//!
//! - [`SpriteTintBuffers::regular`] — applied to normal billboards and actors.
//! - [`SpriteTintBuffers::selflit`] — applied to torches, campfires, braziers
//!   (anything with the `SelfLit` marker) — stays mostly full-bright.
//!
//! `animate_day_cycle` in `game/lighting.rs` writes the new tint values to
//! these buffers on threshold crossings. Since every sprite material holds a
//! clone of the same `Handle`, the GPU buffer is updated in place and all
//! sprites see the new tint on the next frame — with zero material asset
//! mutation and zero iteration over the ECS.

use bevy::prelude::*;
use bevy::render::storage::ShaderStorageBuffer;

/// Holds the shared handles to the two sprite tint buffers.
///
/// Inserted at `Startup` by [`SpriteTintBufferPlugin`], read by every system
/// that creates a sprite material. Callers pick [`regular`](Self::regular) for
/// normal billboards or [`selflit`](Self::selflit) for light sources.
#[derive(Resource, Clone)]
pub struct SpriteTintBuffers {
    pub regular: Handle<ShaderStorageBuffer>,
    pub selflit: Handle<ShaderStorageBuffer>,
}

impl SpriteTintBuffers {
    /// Write new tint values into both buffers. Called by the lighting system
    /// whenever the day/night tint crosses its change threshold. All sprite
    /// materials referencing these handles pick up the new values on the next
    /// render without any per-material mutation.
    pub fn write(&self, buffers: &mut Assets<ShaderStorageBuffer>, regular: Vec4, selflit: Vec4) {
        if let Some(buf) = buffers.get_mut(&self.regular) {
            buf.set_data(regular);
        }
        if let Some(buf) = buffers.get_mut(&self.selflit) {
            buf.set_data(selflit);
        }
    }
}

/// Creates the two shared tint buffers at startup and inserts
/// [`SpriteTintBuffers`] as a resource. Initial value is `Vec4::ONE` (no tint);
/// the lighting system overwrites it on the first frame of `GameState::Game`.
pub struct SpriteTintBufferPlugin;

impl Plugin for SpriteTintBufferPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, init_sprite_tint_buffers);
    }
}

fn init_sprite_tint_buffers(mut commands: Commands, mut buffers: ResMut<Assets<ShaderStorageBuffer>>) {
    let regular = buffers.add(ShaderStorageBuffer::from(Vec4::ONE));
    let selflit = buffers.add(ShaderStorageBuffer::from(Vec4::ONE));
    commands.insert_resource(SpriteTintBuffers { regular, selflit });
}
