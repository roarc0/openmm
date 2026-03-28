#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::apply_pbr_lighting,
    forward_io::VertexOutput,
    mesh_view_bindings::globals,
    pbr_types::PbrInput,
}
#import bevy_pbr::pbr_functions::main_pass_post_lighting_processing

// Water texture bound at slots 100-101 (extension bind group)
@group(#{MATERIAL_BIND_GROUP}) @binding(100)
var water_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101)
var water_sampler: sampler;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> @location(0) vec4<f32> {
    // Get standard PBR input (includes base_color from the atlas texture)
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    // Check if this pixel is a cyan water marker
    let base = pbr_input.material.base_color;
    if base.r < 0.1 && base.g > 0.9 && base.b > 0.9 {
        let t = globals.time;

        // World-position UVs for seamless water tiling
        var water_uv = in.world_position.xz * 0.002;

        // Scroll
        water_uv.x += t * 0.03;
        water_uv.y += t * 0.021;

        // Wave distortion
        water_uv.x += sin(water_uv.y * 6.0 + t * 2.0) * 0.015;
        water_uv.y += cos(water_uv.x * 5.0 + t * 1.5) * 0.015;

        water_uv = fract(water_uv);
        pbr_input.material.base_color = textureSample(water_texture, water_sampler, water_uv);
    }

    // Apply PBR lighting (includes scene lights, shadows, etc.)
    var out_color = apply_pbr_lighting(pbr_input);

    // Apply fog, tonemapping, and other post-processing
    out_color = main_pass_post_lighting_processing(pbr_input, out_color);

    return out_color;
}
