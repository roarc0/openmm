#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::apply_pbr_lighting,
    forward_io::VertexOutput,
    mesh_view_bindings::globals,
    pbr_types::{PbrInput, STANDARD_MATERIAL_FLAGS_UNLIT_BIT},
}
#import bevy_pbr::pbr_functions::main_pass_post_lighting_processing

// Water texture bound at slots 100-101 (extension bind group)
@group(#{MATERIAL_BIND_GROUP}) @binding(100)
var water_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101)
var water_sampler: sampler;

// Water mask (R8): white = water, black = terrain. Nearest-filtered.
@group(#{MATERIAL_BIND_GROUP}) @binding(102)
var water_mask_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(103)
var water_mask_sampler: sampler;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> @location(0) vec4<f32> {
    // Get standard PBR input (includes base_color from the atlas texture)
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    // Sample the water mask at the same UV as the terrain atlas.
    // The mask uses nearest filtering so water edges stay sharp.
    let mask_val = textureSample(water_mask_texture, water_mask_sampler, in.uv).r;

    if mask_val > 0.5 {
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

    var out_color: vec4<f32>;

    // When unlit, skip PBR lighting — display raw texture color (MM6 classic mode)
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) != 0u {
        out_color = pbr_input.material.base_color;
    } else {
        out_color = apply_pbr_lighting(pbr_input);
    }

    // Apply fog, tonemapping, and other post-processing
    out_color = main_pass_post_lighting_processing(pbr_input, out_color);

    return out_color;
}
