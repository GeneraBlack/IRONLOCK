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

struct WaterWaveUniform {
    map_world_size: vec2<f32>,
    time_secs: f32,
    foam_intensity: f32,
    shallow_color: vec4<f32>,
    deep_color: vec4<f32>,
    // x = fow_enabled (0.0 or 1.0)
    fow_params: vec4<f32>,
}

@group(2) @binding(100) var shore_mask_tex: texture_2d<f32>;
@group(2) @binding(101) var shore_mask_sampler: sampler;
@group(2) @binding(102) var<uniform> water_params: WaterWaveUniform;
@group(2) @binding(103) var fow_tex: texture_2d<f32>;
@group(2) @binding(104) var fow_sampler: sampler;

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    let world_xz = in.world_position.xz;
    let safe_size = max(water_params.map_world_size, vec2<f32>(1.0, 1.0));
    let map_uv = clamp(world_xz / safe_size, vec2<f32>(0.001, 0.001), vec2<f32>(0.999, 0.999));

    let shore_sample = textureSample(shore_mask_tex, shore_mask_sampler, map_uv);
    let fow_rgba = textureSample(fow_tex, fow_sampler, map_uv);
    let shore_depth = shore_sample.r;
    let pier_mask = shore_sample.g;
    let t = water_params.time_secs;

    let edge_feather = smoothstep(0.01, 0.16, shore_depth);

    let uv_a = world_xz * 0.68 + vec2<f32>(0.80, 0.60) * (t * 0.58);
    let uv_b = world_xz * 1.24 + vec2<f32>(-0.58, 0.81) * (t * 0.44);
    let uv_c = world_xz * 2.55 + vec2<f32>(0.35, -0.93) * (t * 0.82);

    let dnx = cos(uv_a.x * 2.4 + uv_a.y * 1.7) * 0.14
        + cos(uv_b.x * 3.1 - uv_b.y * 2.2) * 0.10
        + sin(uv_c.x * 4.2 + uv_c.y * 3.6) * 0.05;
    let dnz = sin(uv_a.y * 2.6 - uv_a.x * 1.5) * 0.14
        + sin(uv_b.y * 2.9 + uv_b.x * 2.0) * 0.10
        + cos(uv_c.y * 3.9 - uv_c.x * 3.3) * 0.05;

    let wave_atten = mix(0.35, 1.0, smoothstep(0.02, 0.35, shore_depth));
    let wave_normal = normalize(vec3<f32>(dnx * wave_atten, 1.0, dnz * wave_atten));

    let depth_t = smoothstep(0.06, 0.88, shore_depth);
    var water_rgb = mix(water_params.shallow_color.rgb, water_params.deep_color.rgb, depth_t);

    let surf_wave = 0.5 + 0.5 * sin(shore_depth * 30.0 - t * 2.8 + (dnx + dnz) * 4.5);
    let shore_band = (1.0 - smoothstep(0.03, 0.32, shore_depth))
        * smoothstep(0.005, 0.06, shore_depth)
        * water_params.foam_intensity;
    let pier_ripple = 0.55 + 0.45 * sin(pier_mask * 18.0 - t * 3.4 + (dnx - dnz) * 4.0);
    let pier_foam = pier_mask * pier_ripple * water_params.foam_intensity * 1.15;
    let whitecaps = smoothstep(0.20, 0.30, dnx * 0.6 + dnz * 0.6) * 0.32 * depth_t;
    let foam = clamp(shore_band * (0.38 + 0.65 * surf_wave) + pier_foam + whitecaps, 0.0, 1.0);

    let view_dir = normalize(pbr_input.V);
    let ndotv = clamp(dot(wave_normal, view_dir), 0.0, 1.0);
    let fresnel = pow(1.0 - ndotv, 4.0);
    water_rgb = mix(water_rgb, vec3<f32>(0.54, 0.78, 0.96), fresnel * 0.36);
    water_rgb = mix(water_rgb, vec3<f32>(0.94, 0.98, 1.0), foam * 0.86);

    // Direct GPU World-XZ Fog-of-War darkening on water
    let fow_shroud = clamp(fow_rgba.a * water_params.fow_params.x, 0.0, 0.97);
    let explored_w = smoothstep(0.05, 0.62, fow_shroud);
    let unexplored_w = smoothstep(0.60, 0.94, fow_shroud);
    water_rgb = mix(water_rgb, water_rgb * vec3<f32>(0.34, 0.42, 0.56), explored_w * 0.78);
    water_rgb = mix(water_rgb, vec3<f32>(0.008, 0.015, 0.030), unexplored_w * 0.96);

    let base_alpha = clamp(mix(0.62, 0.94, depth_t) + fresnel * 0.14 + foam * 0.22, 0.0, 0.98);
    let alpha = mix(base_alpha * edge_feather, max(base_alpha * edge_feather, unexplored_w * 0.96), unexplored_w);

    pbr_input.material.base_color = vec4<f32>(water_rgb, alpha);
    pbr_input.material.perceptual_roughness = mix(mix(0.06, 0.40, foam), 0.92, unexplored_w);
    pbr_input.material.metallic = 0.10 * (1.0 - foam) * (1.0 - unexplored_w);
    pbr_input.material.reflectance = mix(0.92, 0.08, unexplored_w);
    pbr_input.N = wave_normal;
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    out.color = vec4<f32>(out.color.rgb * (1.0 - unexplored_w * 0.85), out.color.a);
#endif
    return out;
}
