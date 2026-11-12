#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif

struct TerrainSplatUniform {
    map_world_size: vec2<f32>,
    detail_uv_scale: f32,
    cliff_slope_boost: f32,
    // x = elapsed_secs, y = shadow_strength (0..0.65), z = wind_vel_x, w = wind_vel_z
    cloud_params: vec4<f32>,
    // x = fow_enabled (0.0 or 1.0), y = explored_desat, z = unused, w = unused
    fow_params: vec4<f32>,
}

@group(2) @binding(100) var layer_array_tex: texture_2d_array<f32>;
@group(2) @binding(101) var layer_array_sampler: sampler;
@group(2) @binding(102) var splat_map_tex: texture_2d<f32>;
@group(2) @binding(103) var splat_map_sampler: sampler;
@group(2) @binding(104) var<uniform> terrain_params: TerrainSplatUniform;
@group(2) @binding(105) var normal_rough_height_array_tex: texture_2d_array<f32>;
@group(2) @binding(106) var normal_rough_height_sampler: sampler;
@group(2) @binding(107) var fow_tex: texture_2d<f32>;
@group(2) @binding(108) var fow_sampler: sampler;

struct LayerPbrSample {
    albedo: vec3<f32>,
    normal_xz: vec2<f32>,
    roughness: f32,
    height: f32,
}

fn sample_planar_layer(uv_near: vec2<f32>, uv_macro: vec2<f32>, layer: i32) -> LayerPbrSample {
    let near_c = textureSample(layer_array_tex, layer_array_sampler, uv_near, layer).rgb;
    let macro_c = textureSample(layer_array_tex, layer_array_sampler, uv_macro, layer).rgb;
    let nrh_near = textureSample(
        normal_rough_height_array_tex,
        normal_rough_height_sampler,
        uv_near,
        layer,
    );
    let nrh_macro = textureSample(
        normal_rough_height_array_tex,
        normal_rough_height_sampler,
        uv_macro,
        layer,
    );
    let n_near = nrh_near.rg * 2.0 - vec2<f32>(1.0, 1.0);
    let n_macro = nrh_macro.rg * 2.0 - vec2<f32>(1.0, 1.0);
    var out: LayerPbrSample;
    out.albedo = mix(near_c, macro_c, 0.30);
    out.normal_xz = mix(n_near, n_macro, 0.28);
    out.roughness = mix(nrh_near.b, nrh_macro.b, 0.25);
    out.height = mix(nrh_near.a, nrh_macro.a, 0.30);
    return out;
}

// True 3-Axis Triplanar projection (`ZY` + `XZ` + `XY` weighted by `abs(world_normal)^4`)
// used on steep slopes (`abs(world_normal.y) < 0.78`) so vertical cliffs have zero UV stretching.
fn sample_triplanar_layer(world_pos: vec3<f32>, tri_w: vec3<f32>, scale: f32, layer: i32) -> LayerPbrSample {
    let uv_zy = world_pos.zy * scale;
    let uv_xz = world_pos.xz * scale;
    let uv_xy = world_pos.xy * scale;

    let c_x = textureSample(layer_array_tex, layer_array_sampler, uv_zy, layer).rgb;
    let c_y = textureSample(layer_array_tex, layer_array_sampler, uv_xz, layer).rgb;
    let c_z = textureSample(layer_array_tex, layer_array_sampler, uv_xy, layer).rgb;

    let nrh_x = textureSample(normal_rough_height_array_tex, normal_rough_height_sampler, uv_zy, layer);
    let nrh_y = textureSample(normal_rough_height_array_tex, normal_rough_height_sampler, uv_xz, layer);
    let nrh_z = textureSample(normal_rough_height_array_tex, normal_rough_height_sampler, uv_xy, layer);

    let nx_2d = nrh_x.rg * 2.0 - vec2<f32>(1.0, 1.0);
    let ny_2d = nrh_y.rg * 2.0 - vec2<f32>(1.0, 1.0);
    let nz_2d = nrh_z.rg * 2.0 - vec2<f32>(1.0, 1.0);

    var out: LayerPbrSample;
    out.albedo = c_x * tri_w.x + c_y * tri_w.y + c_z * tri_w.z;
    out.normal_xz = vec2<f32>(
        ny_2d.x * tri_w.y + nz_2d.x * tri_w.z,
        ny_2d.y * tri_w.y + nx_2d.x * tri_w.x,
    );
    out.roughness = nrh_x.b * tri_w.x + nrh_y.b * tri_w.y + nrh_z.b * tri_w.z;
    out.height = nrh_x.a * tri_w.x + nrh_y.a * tri_w.y + nrh_z.a * tri_w.z;
    return out;
}

fn sample_hybrid_layer(
    world_pos: vec3<f32>,
    uv_near: vec2<f32>,
    uv_macro: vec2<f32>,
    tri_w: vec3<f32>,
    tri_blend: f32,
    tri_scale: f32,
    layer: i32,
) -> LayerPbrSample {
    let planar = sample_planar_layer(uv_near, uv_macro, layer);
    let tri = sample_triplanar_layer(world_pos, tri_w, tri_scale, layer);
    var out: LayerPbrSample;
    out.albedo = mix(planar.albedo, tri.albedo, tri_blend);
    out.normal_xz = mix(planar.normal_xz, tri.normal_xz, tri_blend);
    out.roughness = mix(planar.roughness, tri.roughness, tri_blend);
    out.height = mix(planar.height, tri.height, tri_blend);
    return out;
}

fn cloud_hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + vec3<f32>(33.33));
    return fract((p3.x + p3.y) * p3.z);
}

fn cloud_value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (vec2<f32>(3.0) - 2.0 * f);
    let a = cloud_hash21(i);
    let b = cloud_hash21(i + vec2<f32>(1.0, 0.0));
    let c = cloud_hash21(i + vec2<f32>(0.0, 1.0));
    let d = cloud_hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn sample_scrolling_cloud_shadow(world_xz: vec2<f32>, cloud_params: vec4<f32>) -> f32 {
    let strength = clamp(cloud_params.y, 0.0, 0.65);
    if strength <= 0.001 {
        return 0.0;
    }
    let t = cloud_params.x;
    let wind = cloud_params.zw;
    let uv0 = world_xz * 0.024 + wind * t;
    let uv1 = world_xz * 0.051 + wind * (t * 1.38) + vec2<f32>(17.3, 9.1);
    let uv2 = world_xz * 0.105 + wind * (t * 1.75) - vec2<f32>(5.7, 21.4);
    let fbm = cloud_value_noise(uv0) * 0.56
        + cloud_value_noise(uv1) * 0.30
        + cloud_value_noise(uv2) * 0.14;
    return smoothstep(0.45, 0.74, fbm) * strength;
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    let world_pos = in.world_position.xyz;
    let world_xz = world_pos.xz;
    let safe_size = max(terrain_params.map_world_size, vec2<f32>(1.0, 1.0));
    let map_uv = clamp(world_xz / safe_size, vec2<f32>(0.001, 0.001), vec2<f32>(0.999, 0.999));

    let splat_rgba = textureSample(splat_map_tex, splat_map_sampler, map_uv);
    let fow_rgba = textureSample(fow_tex, fow_sampler, map_uv);

    var w0 = splat_rgba.r;
    var w1 = splat_rgba.g;
    var w2 = splat_rgba.b;
    var w3 = splat_rgba.a;
    var w4 = max(0.0, 1.0 - (w0 + w1 + w2 + w3));

    let unit_n = normalize(in.world_normal);
    let abs_n = max(abs(unit_n), vec3<f32>(0.0001));
    var tri_w = pow(abs_n, vec3<f32>(4.0));
    tri_w = tri_w / max(tri_w.x + tri_w.y + tri_w.z, 0.0001);

    let tri_blend = 1.0 - smoothstep(0.60, 0.78, abs_n.y);
    let auto_cliff = (1.0 - smoothstep(0.52, 0.82, abs_n.y)) * terrain_params.cliff_slope_boost;
    w4 = clamp(w4 + auto_cliff * 1.45, 0.0, 1.65);

    let raw_sum = max(w0 + w1 + w2 + w3 + w4, 0.0001);
    w0 = w0 / raw_sum;
    w1 = w1 / raw_sum;
    w2 = w2 / raw_sum;
    w3 = w3 / raw_sum;
    w4 = w4 / raw_sum;

    let uv_near = world_xz * terrain_params.detail_uv_scale;
    let uv_macro = vec2<f32>(
        world_xz.x * 0.8660254 - world_xz.y * 0.5,
        world_xz.x * 0.5 + world_xz.y * 0.8660254,
    ) * (terrain_params.detail_uv_scale * 0.27);
    let tri_scale = terrain_params.detail_uv_scale * 0.92;

    let s0 = sample_hybrid_layer(world_pos, uv_near, uv_macro, tri_w, tri_blend, tri_scale, 0);
    let s1 = sample_hybrid_layer(world_pos, uv_near, uv_macro, tri_w, tri_blend, tri_scale, 1);
    let s2 = sample_hybrid_layer(world_pos, uv_near, uv_macro, tri_w, tri_blend, tri_scale, 2);
    let s3 = sample_hybrid_layer(world_pos, uv_near, uv_macro, tri_w, tri_blend, tri_scale, 3);
    let s4 = sample_hybrid_layer(world_pos, uv_near * 0.85, uv_macro, tri_w, tri_blend, tri_scale * 0.85, 4);

    let h_depth = 0.24;
    let p0 = w0 * 1.25 + s0.height * 0.52;
    let p1 = w1 * 1.25 + s1.height * 0.52;
    let p2 = w2 * 1.25 + s2.height * 0.52;
    let p3 = w3 * 1.25 + s3.height * 0.52;
    let p4 = w4 * 1.25 + s4.height * 0.58;
    let max_p = max(max(max(p0, p1), max(p2, p3)), p4) - h_depth;

    var bw0 = max(p0 - max_p, 0.0) * clamp(w0 * 8.0, 0.0, 1.0);
    var bw1 = max(p1 - max_p, 0.0) * clamp(w1 * 8.0, 0.0, 1.0);
    var bw2 = max(p2 - max_p, 0.0) * clamp(w2 * 8.0, 0.0, 1.0);
    var bw3 = max(p3 - max_p, 0.0) * clamp(w3 * 8.0, 0.0, 1.0);
    var bw4 = max(p4 - max_p, 0.0) * clamp(w4 * 8.0, 0.0, 1.0);
    let bw_sum = max(bw0 + bw1 + bw2 + bw3 + bw4, 0.0001);
    bw0 = bw0 / bw_sum;
    bw1 = bw1 / bw_sum;
    bw2 = bw2 / bw_sum;
    bw3 = bw3 / bw_sum;
    bw4 = bw4 / bw_sum;

    var albedo = s0.albedo * bw0
        + s1.albedo * bw1
        + s2.albedo * bw2
        + s3.albedo * bw3
        + s4.albedo * bw4;

    let height_tint = clamp(0.95 + world_pos.y * 0.045, 0.82, 1.12);
    albedo = albedo * height_tint;

    let cloud_shadow = sample_scrolling_cloud_shadow(world_xz, terrain_params.cloud_params);
    albedo = mix(albedo, albedo * vec3<f32>(0.52, 0.58, 0.70), cloud_shadow);

    // Direct GPU World-XZ Fog-of-War projection (zero floating plane parallax on cliffs & ramps!)
    let fow_shroud = clamp(fow_rgba.a * terrain_params.fow_params.x, 0.0, 0.97);
    let luma = dot(albedo, vec3<f32>(0.299, 0.587, 0.114));
    let explored_cool = vec3<f32>(luma * 0.34, luma * 0.40, luma * 0.54);
    let explored_w = smoothstep(0.05, 0.62, fow_shroud);
    let unexplored_w = smoothstep(0.60, 0.94, fow_shroud);
    albedo = mix(albedo, explored_cool, explored_w * 0.76);
    albedo = mix(albedo, vec3<f32>(0.009, 0.014, 0.026), unexplored_w * 0.95);

    let detail_n = s0.normal_xz * (bw0 * 0.28)
        + s1.normal_xz * (bw1 * 0.42)
        + s2.normal_xz * (bw2 * 0.30)
        + s3.normal_xz * (bw3 * 0.22)
        + s4.normal_xz * (bw4 * 0.65);
    let blended_roughness = clamp(
        s0.roughness * bw0
            + s1.roughness * bw1
            + s2.roughness * bw2
            + s3.roughness * bw3
            + s4.roughness * bw4,
        0.12,
        0.97,
    );

    pbr_input.material.base_color = vec4<f32>(albedo, 1.0);
    pbr_input.material.perceptual_roughness = mix(blended_roughness, 0.95, unexplored_w);
    pbr_input.material.metallic = (0.04 * bw3 + 0.07 * bw4) * (1.0 - unexplored_w);
    pbr_input.N = normalize(pbr_input.N + vec3<f32>(detail_n.x, 0.0, detail_n.y));
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    // Attenuate post-IBL specular bounce inside unexplored shroud so unrevealed terrain stays mysterious
    out.color = vec4<f32>(out.color.rgb * (1.0 - unexplored_w * 0.86), out.color.a);
#endif
    return out;
}
