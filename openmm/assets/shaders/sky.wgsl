#import bevy_pbr::{
    mesh_functions,
    view_transformations,
    mesh_view_bindings::globals,
    mesh_view_bindings::view,
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
    @location(1) world_position: vec3<f32>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> sun_dir: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1)
var sky_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2)
var sky_sampler: sampler;

@vertex
fn vertex(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_position = mesh_functions::get_world_from_local(vertex.instance_index) * vec4<f32>(vertex.position, 1.0);
    out.clip_position = view_transformations::position_world_to_clip(world_position.xyz);
    out.uv = vertex.uv;
    out.world_position = world_position.xyz;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var uv = in.uv;
    let t = globals.time;
    uv.x += t * 0.003;
    uv.y += t * 0.001;
    var color = textureSample(sky_texture, sky_sampler, uv);

    // Fade sky to transparent near the horizon.
    // Use the viewing angle: steeper = overhead (full opacity),
    // shallow = horizon (fade to transparent, ClearColor/fog shows through).
    let cam_pos = view.world_position.xyz;
    let to_frag = in.world_position - cam_pos;
    let dir = normalize(to_frag);

    // dir.y > 0 means looking up. Fade out when dir.y is small (near horizon).
    let fade = smoothstep(0.0, 0.15, abs(dir.y));
    color.a = fade;

    // Visible sun disc — locked to the same direction the directional light uses,
    // so the disc on screen matches where shadows say the sun is.
    // Only draw when the sun is above the horizon and the fragment is on the
    // upper hemisphere of the sky plane (dir.y > 0).
    // sun_dir.w is the visibility flag from `cfg.visible_sun` (0.0 hides the disc).
    let sun = normalize(sun_dir.xyz);
    if (sun_dir.w > 0.5 && sun.y > 0.0 && dir.y > 0.0) {
        let d = dot(dir, sun);
        // Tight disc + soft halo. Tuned for a small MM6-ish sun.
        let disc = smoothstep(0.9995, 0.9999, d);
        let halo = smoothstep(0.985, 1.0, d) * 0.35;
        // Warm at horizon, white at zenith.
        let warmth = 1.0 - sun.y;
        let sun_color = vec3<f32>(1.0, 0.85 + 0.15 * sun.y, 0.65 + 0.35 * sun.y);
        let sun_rgb = sun_color * (disc + halo);
        color = vec4<f32>(max(color.rgb, sun_rgb), max(color.a, (disc + halo) * fade));
    }

    return color;
}
