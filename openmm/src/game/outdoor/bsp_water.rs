//! Animated water material for BSP model faces (fountains, pools, canals).
//!
//! MM6's terrain water is animated via the [`super::spawn_terrain::WaterExtension`]
//! shader which reads a per-pixel mask over the terrain atlas. That design
//! doesn't apply to BSP sub-meshes: each sub-mesh carries exactly one
//! texture, so there's nothing to mask — the whole face is water or it
//! isn't.
//!
//! This module provides a stripped-down sibling material:
//!
//!   * `BspWaterMaterial = ExtendedMaterial<StandardMaterial, BspWaterExtension>`
//!   * No extra bindings; the extension is a zero-sized tag.
//!   * The fragment shader scrolls and distorts the incoming UVs using
//!     `globals.time` before calling `pbr_input_from_standard_material`, so
//!     the face's existing `base_color_texture` (the water texture itself)
//!     gets sampled with the same scroll/wave maths as terrain water.
//!
//! Sub-meshes whose texture name matches [`is_bsp_water_texture`] are spawned
//! with this material instead of a plain `StandardMaterial` — see
//! [`super::bsp::spawn_bsp_models`].

use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

/// Full BSP water material: plain `StandardMaterial` (with the water texture
/// bound as its `base_color_texture`) plus a time-driven UV distortion
/// fragment shader.
pub type BspWaterMaterial = ExtendedMaterial<StandardMaterial, BspWaterExtension>;

/// Zero-sized extension tag. The fragment shader needs no per-material
/// data — everything it needs (the water texture, PBR state) is already in
/// the `StandardMaterial` base, and the animation input is `globals.time`.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct BspWaterExtension {}

impl MaterialExtension for BspWaterExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/bsp_water.wgsl".into()
    }
}

/// True when the given BSP sub-mesh texture name is one of MM6's animated
/// water tiles. Case-insensitive substring match so both `WtrTyl` and
/// `wtrtyl1` (if it ever shows up) hit.
///
/// The set is intentionally narrow — right now only `WtrTyl` (the standard
/// outdoor fountain / pool water tile) qualifies. Other MM6 water textures
/// (`OrWtr*`, `SWtr*`, `Wtrdr*` directional pieces) render as static because
/// their UVs are authored for specific geometry and scrolling them would
/// break alignment. Extend this predicate if a specific texture needs
/// animation, rather than broadening the pattern.
pub fn is_bsp_water_texture(name: &str) -> bool {
    name.to_ascii_lowercase().contains("wtrtyl")
}
