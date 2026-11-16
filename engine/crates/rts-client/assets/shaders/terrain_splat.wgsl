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
    normal_3d: vec3<f32>,
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
    // Keep axis-aligned RA2 concrete slab & asphalt lane markings (layer 3) un-rotated
    if layer == 3 {
        let macro_luma = dot(macro_c, vec3<f32>(0.299, 0.587, 0.114));
        out.albedo = clamp(near_c * (0.93 + macro_luma * 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
        out.normal_3d = vec3<f32>(n_near.x, 0.0, n_near.y);
        out.roughness = mix(nrh_near.b, nrh_macro.b, 0.14);
        out.height = nrh_near.a;
    } else {
        let nxz = mix(n_near, n_macro, 0.22);
        out.albedo = mix(near_c, macro_c, 0.22);
        out.normal_3d = vec3<f32>(nxz.x, 0.0, nxz.y);
        out.roughness = mix(nrh_near.b, nrh_macro.b, 0.20);
        out.height = mix(nrh_near.a, nrh_macro.a, 0.22);
    }
    return out;
}

// Pure World-ZY + World-XY Triplanar Cliff-Face Sampler:
// Eliminates 100% of vertical UV stretching on stepped `2.20m` RA2 cliffs (`1.0 - world_normal.y > 0.18`)
// and reconstructs full 3D rock-ledge normal perturbations (including vertical Y shelf tilt!).
fn sample_cliff_wall_triplanar(world_pos: vec3<f32>, unit_n: vec3<f32>, scale: f32) -> LayerPbrSample {
    let horiz = max(abs(unit_n.xz), vec2<f32>(0.001));
    let w_p = pow(horiz, vec2<f32>(4.0));
    let wall_w = w_p / max(w_p.x + w_p.y, 0.0001);

    // Dual-scale world-ZY & world-XY rock strata UVs (`-world_pos.y` keeps sunlit rock ledges facing skyward)
    let strata_warp = sin(world_pos.x * 0.42 + world_pos.z * 0.37) * 0.09;
    let uv_zy_near = vec2<f32>(world_pos.z, -world_pos.y + strata_warp) * (scale * 1.36);
    let uv_xy_near = vec2<f32>(world_pos.x, -world_pos.y - strata_warp) * (scale * 1.36);
    let uv_zy_macro = vec2<f32>(world_pos.z * 0.43 + 0.17, (-world_pos.y) * 0.47 + 0.31) * scale;
    let uv_xy_macro = vec2<f32>(world_pos.x * 0.43 + 0.41, (-world_pos.y) * 0.47 + 0.13) * scale;

    let cx_near = textureSample(layer_array_tex, layer_array_sampler, uv_zy_near, 4).rgb;
    let cz_near = textureSample(layer_array_tex, layer_array_sampler, uv_xy_near, 4).rgb;
    let cx_mac = textureSample(layer_array_tex, layer_array_sampler, uv_zy_macro, 4).rgb;
    let cz_mac = textureSample(layer_array_tex, layer_array_sampler, uv_xy_macro, 4).rgb;

    let nrh_x_near = textureSample(normal_rough_height_array_tex, normal_rough_height_sampler, uv_zy_near, 4);
    let nrh_z_near = textureSample(normal_rough_height_array_tex, normal_rough_height_sampler, uv_xy_near, 4);
    let nrh_x_mac = textureSample(normal_rough_height_array_tex, normal_rough_height_sampler, uv_zy_macro, 4);
    let nrh_z_mac = textureSample(normal_rough_height_array_tex, normal_rough_height_sampler, uv_xy_macro, 4);

    let cx = mix(cx_near, cx_mac, 0.24);
    let cz = mix(cz_near, cz_mac, 0.24);
    let nrh_x = mix(nrh_x_near, nrh_x_mac, 0.22);
    let nrh_z = mix(nrh_z_near, nrh_z_mac, 0.22);

    // Unpack 2D tangent derivatives: .r = horizontal along wall, .g = vertical along -world_pos.y
    let tx = nrh_x.rg * 2.0 - vec2<f32>(1.0, 1.0);
    let tz = nrh_z.rg * 2.0 - vec2<f32>(1.0, 1.0);

    // Map into 3D world-space normal offset (`-t.y` lifts upper rock ledges toward +Y sky!)
    let dn_zy = vec3<f32>(0.0, -tx.y, tx.x);
    let dn_xy = vec3<f32>(tz.x, -tz.y, 0.0);

    var out: LayerPbrSample;
    out.albedo = cx * wall_w.x + cz * wall_w.y;
    out.normal_3d = dn_zy * wall_w.x + dn_xy * wall_w.y;
    out.roughness = nrh_x.b * wall_w.x + nrh_z.b * wall_w.y;
    out.height = nrh_x.a * wall_w.x + nrh_z.a * wall_w.y;
    return out;
}

fn sample_triplanar_layer(world_pos: vec3<f32>, tri_w: vec3<f32>, scale: f32, layer: i32) -> LayerPbrSample {
    let uv_zy = vec2<f32>(world_pos.z, -world_pos.y) * scale;
    let uv_xz = world_pos.xz * scale;
    let uv_xy = vec2<f32>(world_pos.x, -world_pos.y) * scale;

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
    out.normal_3d = vec3<f32>(
        ny_2d.x * tri_w.y + nz_2d.x * tri_w.z,
        (-nx_2d.y) * tri_w.x + (-nz_2d.y) * tri_w.z,
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
    out.normal_3d = mix(planar.normal_3d, tri.normal_3d, tri_blend);
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

// Crisp sub-pixel RA2 modular concrete base-pad grid micro-relief (`2m` major tile slabs + `1m` sub-slabs)
fn sample_ra2_concrete_grid_accent(world_xz: vec2<f32>) -> f32 {
    let major_f = fract(world_xz * 0.5);
    let minor_f = fract(world_xz * 1.0);
    let d_major = min(min(major_f.x, 1.0 - major_f.x), min(major_f.y, 1.0 - major_f.y));
    let d_minor = min(min(minor_f.x, 1.0 - minor_f.x), min(minor_f.y, 1.0 - minor_f.y));
    let major_groove = 1.0 - smoothstep(0.008, 0.024, d_major);
    let minor_groove = 1.0 - smoothstep(0.006, 0.018, d_minor);
    let bevel = (1.0 - smoothstep(0.024, 0.050, d_major)) * (1.0 - major_groove);
    return 1.0 - major_groove * 0.22 - minor_groove * 0.09 + bevel * 0.06;
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

    var vert_curv = vec4<f32>(0.0, 0.0, 0.0, 1.0);
#ifdef VERTEX_COLORS
    // Mesh::ATTRIBUTE_COLOR carries [plateau_rim_hi, cliff_toe_ao, ramp_weight, 1.0] on chunk meshes
    // (or [1,1,1,1] on legacy test meshes — detect legacy all-ones safely via vert_curv.r*vert_curv.g < 0.8)
    if !(in.color.r > 0.98 && in.color.g > 0.98 && in.color.b > 0.98) {
        vert_curv = in.color;
    }
#endif
    let rim_curv = clamp(vert_curv.r, 0.0, 1.0);
    let toe_curv = clamp(vert_curv.g, 0.0, 1.0);
    let ramp_mask = clamp(vert_curv.b, 0.0, 1.0);

    var w0 = splat_rgba.r;
    var w1 = splat_rgba.g;
    var w2 = splat_rgba.b;
    var w3 = splat_rgba.a;
    var w4 = max(0.0, 1.0 - (w0 + w1 + w2 + w3));

    let unit_n = normalize(in.world_normal);
    let abs_n = max(abs(unit_n), vec3<f32>(0.0001));
    var tri_w = pow(abs_n, vec3<f32>(6.0));
    tri_w = tri_w / max(tri_w.x + tri_w.y + tri_w.z, 0.0001);

    // Steep slope normal detection (`1.0 - world_normal.y > 0.18`):
    // Escarpments blend crisply into pure world-XY / world-ZY triplanar rock cliff ledges,
    // while gentle navigable ramps (`ramp_mask > 0.5`) stay driveable graded dirt/stone.
    let slope_delta = 1.0 - clamp(unit_n.y, 0.0, 1.0);
    let raw_steep = smoothstep(0.18, 0.34, slope_delta);
    let slope_cliff = raw_steep * mix(1.0, smoothstep(0.34, 0.52, slope_delta), ramp_mask * 0.85);
    let tri_blend = smoothstep(0.16, 0.36, slope_delta);

    let auto_cliff = slope_cliff * terrain_params.cliff_slope_boost * 2.15;
    let flat_keep = 1.0 - clamp(slope_cliff * 0.96, 0.0, 0.96);
    w0 = w0 * flat_keep;
    w1 = w1 * mix(1.0, 0.18, slope_cliff);
    w2 = w2 * flat_keep;
    w3 = w3 * flat_keep;
    w4 = clamp(w4 + auto_cliff, 0.0, 3.20);

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
    ) * (terrain_params.detail_uv_scale * 0.29);
    let tri_scale = terrain_params.detail_uv_scale * 1.25;

    let s0 = sample_hybrid_layer(world_pos, uv_near, uv_macro, tri_w, tri_blend, tri_scale, 0);
    let s1 = sample_hybrid_layer(world_pos, uv_near, uv_macro, tri_w, tri_blend, tri_scale, 1);
    let s2 = sample_hybrid_layer(world_pos, uv_near, uv_macro, tri_w, tri_blend, tri_scale, 2);
    let s3 = sample_hybrid_layer(world_pos, uv_near, uv_macro, tri_w, tri_blend, tri_scale, 3);

    // Pure zero-stretch world-XY / world-ZY triplanar cliff stone blended seamlessly onto plateau lip
    let s4_planar = sample_planar_layer(uv_near, uv_macro, 4);
    let s4_wall = sample_cliff_wall_triplanar(world_pos, unit_n, terrain_params.detail_uv_scale);
    let wall_lock = clamp(max(tri_blend, slope_cliff * 1.15), 0.0, 1.0);
    var s4: LayerPbrSample;
    s4.albedo = mix(s4_planar.albedo, s4_wall.albedo, wall_lock);
    s4.normal_3d = mix(s4_planar.normal_3d, s4_wall.normal_3d, wall_lock);
    s4.roughness = mix(s4_planar.roughness, s4_wall.roughness, wall_lock);
    s4.height = mix(s4_planar.height, s4_wall.height, wall_lock);

    // Tight height-blended splat transitions (razor-sharp jagged stone edge where cliff meets grass/sand!)
    let h_depth = mix(0.13, 0.075, clamp(w3 + w4, 0.0, 1.0));
    let p0 = w0 * 1.32 + s0.height * 0.48;
    let p1 = w1 * 1.32 + s1.height * 0.52;
    let p2 = w2 * 1.32 + s2.height * 0.48;
    let p3 = w3 * 1.40 + s3.height * 0.56;
    let p4 = w4 * 1.44 + s4.height * 0.68;
    let max_p = max(max(max(p0, p1), max(p2, p3)), p4) - h_depth;

    var bw0 = max(p0 - max_p, 0.0) * clamp(w0 * 10.0, 0.0, 1.0);
    var bw1 = max(p1 - max_p, 0.0) * clamp(w1 * 10.0, 0.0, 1.0);
    var bw2 = max(p2 - max_p, 0.0) * clamp(w2 * 10.0, 0.0, 1.0);
    var bw3 = max(p3 - max_p, 0.0) * clamp(w3 * 12.0, 0.0, 1.0);
    var bw4 = max(p4 - max_p, 0.0) * clamp(w4 * 14.0, 0.0, 1.0);
    let bw_sum = max(bw0 + bw1 + bw2 + bw3 + bw4, 0.0001);
    bw0 = bw0 / bw_sum;
    bw1 = bw1 / bw_sum;
    bw2 = bw2 / bw_sum;
    bw3 = bw3 / bw_sum;
    bw4 = bw4 / bw_sum;

    let slab_accent = sample_ra2_concrete_grid_accent(world_xz);
    let paved_albedo = s3.albedo * slab_accent;

    // Deep crevice AO / cavity darkening + crisp sunlit plateau-rim edge highlight on 2.20m cliffs
    let crevice_ao = mix(0.46, 1.15, smoothstep(0.10, 0.86, s4.height));
    let rim_band = max(
        rim_curv,
        smoothstep(0.06, 0.20, slope_delta) * (1.0 - smoothstep(0.24, 0.46, slope_delta)) * (1.0 - ramp_mask),
    );
    let rim_edge_hi = rim_band * smoothstep(0.38, 0.85, s4.height) * 0.20;
    let toe_cavity_ao = 1.0 - toe_curv * 0.26;
    let cliff_albedo = (s4.albedo * crevice_ao + vec3<f32>(rim_edge_hi * 1.05, rim_edge_hi * 0.98, rim_edge_hi * 0.86)) * toe_cavity_ao;

    var albedo = s0.albedo * bw0
        + s1.albedo * bw1
        + s2.albedo * bw2
        + paved_albedo * bw3
        + cliff_albedo * bw4;

    // Subtle curb contact shadow along concrete-to-ground slab edges & cliff base contact
    let curb_ao = 1.0 - clamp(bw3 * (bw0 + bw1 + bw2) * 3.6, 0.0, 1.0) * 0.16;
    albedo = albedo * curb_ao * mix(1.0, toe_cavity_ao, 0.45);

    // Subtle elevation sun-lift + plateau-lip bevel accent so 2.20m plateaus pop over valleys
    let height_tint = clamp(0.95 + world_pos.y * 0.052, 0.82, 1.15);
    albedo = albedo * height_tint + vec3<f32>(rim_band * (1.0 - bw4) * 0.045);

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

    let detail_n3 = s0.normal_3d * (bw0 * 0.34)
        + s1.normal_3d * (bw1 * 0.48)
        + s2.normal_3d * (bw2 * 0.32)
        + s3.normal_3d * (bw3 * 0.38)
        + s4.normal_3d * (bw4 * 0.95);
    let cliff_rough = clamp(s4.roughness - rim_band * 0.10, 0.38, 0.95);
    let blended_roughness = clamp(
        s0.roughness * bw0
            + s1.roughness * bw1
            + s2.roughness * bw2
            + s3.roughness * bw3
            + cliff_rough * bw4,
        0.12,
        0.97,
    );

    pbr_input.material.base_color = vec4<f32>(albedo, 1.0);
    pbr_input.material.perceptual_roughness = mix(blended_roughness, 0.95, unexplored_w);
    pbr_input.material.metallic = (0.06 * bw3 + 0.09 * bw4) * (1.0 - unexplored_w);
    pbr_input.N = normalize(pbr_input.N + detail_n3);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    out.color = vec4<f32>(out.color.rgb * (1.0 - unexplored_w * 0.86), out.color.a);
#endif
    return out;
}
