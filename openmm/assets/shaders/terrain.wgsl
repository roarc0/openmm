#import bevy_pbr::{
    mesh_functions,
    view_transformations,
    mesh_view_bindings::globals,
}

struct VertexInput {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) world_position: vec3<f32>,
};

struct TerrainParams {
    base_color: vec4<f32>,
    time: f32,
    water_speed: f32,
    water_distortion: f32,
    _padding: f32,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> params: TerrainParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(1)
var terrain_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2)
var terrain_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(3)
var water_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4)
var water_sampler: sampler;

@vertex
fn vertex(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_position = mesh_functions::get_world_from_local(vertex.instance_index) * vec4<f32>(vertex.position, 1.0);
    out.clip_position = view_transformations::position_world_to_clip(world_position.xyz);
    out.world_position = world_position.xyz;
    out.uv = vertex.uv;
    out.world_normal = (mesh_functions::get_world_from_local(vertex.instance_index) * vec4<f32>(vertex.normal, 0.0)).xyz;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the raw atlas color BEFORE applying base_color tint
    let raw = textureSample(terrain_texture, terrain_sampler, in.uv);

    // Detect cyan water markers with smooth blending at edges.
    // Bilinear filtering blends cyan with neighboring terrain, so we use
    // a smooth factor instead of a hard threshold.
    // With nearest-neighbor filtering, cyan pixels don't bleed into terrain.
    // Simple threshold is sufficient.
    let is_cyan = raw.r < 0.1 && raw.g > 0.9 && raw.b > 0.9;
    let water_blend = select(0.0, 1.0, is_cyan);

    var color: vec4<f32>;
    if water_blend > 0.01 {
        let t = globals.time;

        // World-position UVs for seamless water tiling
        var water_uv = in.world_position.xz * 0.002;

        // Scroll
        water_uv.x += t * params.water_speed;
        water_uv.y += t * params.water_speed * 0.7;

        // Wave distortion
        water_uv.x += sin(water_uv.y * 6.0 + t * 2.0) * params.water_distortion;
        water_uv.y += cos(water_uv.x * 5.0 + t * 1.5) * params.water_distortion;

        water_uv = fract(water_uv);
        let water_color = textureSample(water_texture, water_sampler, water_uv);
        let terrain_color = raw * params.base_color;

        // Smooth blend between terrain and water at tile boundaries
        color = mix(terrain_color, water_color, water_blend);
    } else {
        color = raw * params.base_color;
    }

    // Simple directional lighting
    let light_dir = normalize(vec3<f32>(0.3, 0.8, 0.3));
    let ndotl = max(dot(normalize(in.world_normal), light_dir), 0.0);
    let ambient = 0.35;
    let lit = color.rgb * (ambient + (1.0 - ambient) * ndotl);

    return vec4<f32>(lit, 1.0);
}
