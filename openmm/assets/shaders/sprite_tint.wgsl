#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, alpha_discard},
    forward_io::VertexOutput,
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
}
#import bevy_pbr::pbr_functions::main_pass_post_lighting_processing

// Globally-shared day/night tint, updated once per frame on the CPU via
// `sprites::tint_buffer::write_sprite_globals` (render world) and uploaded
// with `queue.write_buffer`. Every sprite material's bind group references
// the same wgpu Buffer — see `sprites/material.rs` for the hand-written
// `AsBindGroup` impl that wires this binding without going through Bevy's
// per-material asset pipeline.
//
// `regular` is the ambient day/night tint that most billboards use; `selflit`
// is a lighter tint applied to torches, campfires, braziers and other light
// sources so they stay grounded in the scene without being dimmed at night.
// Selection is per-material: `SpriteExtension::selflit` is surfaced as
// `AsBindGroup::Data = u32` and turned into the `SPRITE_SELFLIT` shader def
// in `MaterialExtension::specialize`, producing two pipeline variants.
//
// TODO: add point-light contribution from nearby torches/campfires in dungeons
//       so sprites respond to indoor lighting rather than only the global ambient tint.
struct SpriteGlobals {
    regular: vec4<f32>,
    selflit: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100)
var<uniform> sprite_globals: SpriteGlobals;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> @location(0) vec4<f32> {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

#ifdef SPRITE_SELFLIT
    let tint = sprite_globals.selflit;
#else
    let tint = sprite_globals.regular;
#endif

    // Sprites are unlit (billboard normals always face the camera, making directional
    // lighting flicker with camera rotation). Tint simulates ambient day/night variation.
    pbr_input.material.base_color *= tint;
    // Discard transparent pixels (AlphaMode::Mask) before any output.
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

    var out_color: vec4<f32>;
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) != 0u {
        out_color = pbr_input.material.base_color;
    } else {
        out_color = apply_pbr_lighting(pbr_input);
    }

    out_color = main_pass_post_lighting_processing(pbr_input, out_color);
    return out_color;
}
