#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, alpha_discard},
    forward_io::VertexOutput,
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
}
#import bevy_pbr::pbr_functions::main_pass_post_lighting_processing

// Day/night tint for this sprite (linear sRGB). Vec4(1.0) = no change.
//
// Per-material uniform, rewritten by `animate_day_cycle` on threshold
// crossings. A previous revision (A1 / commit 0676f4f) routed this through a
// shared `ShaderStorageBuffer` asset to avoid iterating every sprite
// material, but Bevy 0.18 does not invalidate material bind groups when a
// referenced storage buffer asset changes, so the tint stopped propagating.
// The per-material uniform is the design that actually works on 0.18.
//
// TODO: add point-light contribution from nearby torches/campfires in dungeons
//       so sprites respond to indoor lighting rather than only the global ambient tint.
@group(#{MATERIAL_BIND_GROUP}) @binding(100)
var<uniform> tint: vec4<f32>;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> @location(0) vec4<f32> {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

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
