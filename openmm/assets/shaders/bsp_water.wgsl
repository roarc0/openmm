// Animated water shader for BSP sub-meshes that carry the WtrTyl tile.
//
// No custom bindings — the sub-mesh's StandardMaterial already has the water
// texture set as its base_color_texture. We just scroll and distort the
// incoming UVs using globals.time, then hand the modified VertexOutput to
// pbr_input_from_standard_material so the rest of the PBR pipeline (fog,
// lighting, tonemapping) behaves exactly the same as every other BSP face.
//
// The scroll/wave constants match assets/shaders/water.wgsl (terrain water)
// so fountains and the surrounding lakes ripple at the same tempo.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, alpha_discard},
    forward_io::VertexOutput,
    mesh_view_bindings::globals,
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
}
#import bevy_pbr::pbr_functions::main_pass_post_lighting_processing

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> @location(0) vec4<f32> {
    let t = globals.time;

    // Scroll the UVs, then overlay small sinusoidal waves. Values tuned to
    // match the terrain water shader so both systems feel the same.
    var uv = in.uv;
    uv.x += t * 0.03;
    uv.y += t * 0.021;
    uv.x += sin(uv.y * 6.0 + t * 2.0) * 0.015;
    uv.y += cos(uv.x * 5.0 + t * 1.5) * 0.015;
    uv = fract(uv);

    // Feed the modified UV back through the standard PBR input so we reuse
    // all of StandardMaterial's sampling/emissive/alpha logic.
    var in_mut = in;
    in_mut.uv = uv;
    var pbr_input = pbr_input_from_standard_material(in_mut, is_front);

    var out_color: vec4<f32>;
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) != 0u {
        out_color = pbr_input.material.base_color;
    } else {
        out_color = apply_pbr_lighting(pbr_input);
    }
    out_color = main_pass_post_lighting_processing(pbr_input, out_color);
    return out_color;
}
