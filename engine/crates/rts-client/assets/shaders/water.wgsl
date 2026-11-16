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

fn water_hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + vec3<f32>(33.33));
    return fract((p3.x + p3.y) * p3.z);
}

fn water_noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (vec2<f32>(3.0) - 2.0 * f);
    let a = water_hash21(i);
    let b = water_hash21(i + vec2<f32>(1.0, 0.0));
    let c = water_hash21(i + vec2<f32>(0.0, 1.0));
    let d = water_hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// Sharpened Gerstner wave derivative `(dnx, dnz, crest_h)` for realistic peaked wave crests
fn gerstner_octave(pos: vec2<f32>, dir: vec2<f32>, freq: f32, speed: f32, t: f32, amp: f32) -> vec3<f32> {
    let phase = dot(pos, dir) * freq - t * speed;
    let s = sin(phase);
    let c = cos(phase);
    let sharp = 1.0 + 0.42 * s;
    return vec3<f32>(-dir.x * c * amp * sharp, -dir.y * c * amp * sharp, s * amp);
}

// Refracted Voronoi-interference seabed caustic filament network
fn sample_caustic_lattice(uv: vec2<f32>) -> f32 {
    let p = uv * 3.14159265;
    let gx = abs(sin(p.x + cos(p.y * 1.27) * 0.82));
    let gy = abs(sin(p.y - cos(p.x * 1.31) * 0.78));
    let ridge = 1.0 - min(gx, gy);
    return pow(clamp(ridge, 0.0, 1.0), 4.5);
}

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
    let pier_cliff_mask = shore_sample.g;
    let naval_hull_mask = shore_sample.b;
    let t = water_params.time_secs;

    let edge_feather = smoothstep(0.006, 0.11, shore_depth);

    // Dual counter-scrolling multi-octave Gerstner swells + fine capillary wind ripples
    let g0 = gerstner_octave(world_xz, vec2<f32>(0.80, 0.60), 1.65, 1.55, t, 0.14);
    let g1 = gerstner_octave(world_xz, vec2<f32>(-0.56, 0.83), 2.55, 1.90, t, 0.11);
    let g2 = gerstner_octave(world_xz, vec2<f32>(0.38, -0.92), 4.40, 2.65, t, 0.065);
    let g3 = gerstner_octave(world_xz, vec2<f32>(-0.88, -0.47), 7.60, 3.40, t, 0.038);

    let dnx = g0.x + g1.x + g2.x + g3.x;
    let dnz = g0.y + g1.y + g2.y + g3.y;
    let crest_h = g0.z + g1.z + g2.z * 0.7;

    let wave_atten = mix(0.34, 1.0, smoothstep(0.02, 0.32, shore_depth));
    let wave_normal = normalize(vec3<f32>(dnx * wave_atten, 1.0, dnz * wave_atten));

    // Optical shallow-to-deep sapphire water absorption + dual counter-scrolling seabed caustics
    let depth_t = smoothstep(0.05, 0.80, shore_depth);
    var water_rgb = mix(water_params.shallow_color.rgb, water_params.deep_color.rgb, depth_t);

    let refract_uv = world_xz * 0.58 + wave_normal.xz * 0.24;
    let c_a = sample_caustic_lattice(refract_uv + vec2<f32>(0.21, 0.14) * t);
    let c_b = sample_caustic_lattice(refract_uv * 1.27 - vec2<f32>(0.16, -0.22) * t + vec2<f32>(1.7, 2.3));
    let caustic = clamp((c_a * 0.65 + c_b * 0.65 + c_a * c_b * 1.4), 0.0, 1.25)
        * (1.0 - smoothstep(0.12, 0.82, shore_depth))
        * smoothstep(0.01, 0.08, shore_depth);
    water_rgb = water_rgb + vec3<f32>(0.14, 0.36, 0.42) * (caustic * 0.42);

    // Organic cellular sea-foam lace texture for realistic frothy shoreline & hull bubbles
    let foam_lace = mix(
        0.62,
        1.22,
        water_noise2(world_xz * 3.8 + vec2<f32>(t * 0.35, -t * 0.28)) * 0.65
            + water_noise2(world_xz * 8.4 - vec2<f32>(t * 0.52, t * 0.41)) * 0.35,
    );

    // Animated 2-band shoreline wave foam lapping against beaches, cliffs, piers & naval hulls:
    // Band 1: Crisp inner beach/cliff contact swash line
    let swash_shift = 0.012 * sin(t * 2.4 + world_xz.x * 0.9 + world_xz.y * 0.7);
    let contact_foam = (1.0 - smoothstep(0.022, 0.125 + swash_shift, shore_depth))
        * smoothstep(0.003, 0.032, shore_depth)
        * foam_lace
        * water_params.foam_intensity;

    // Band 2: Secondary shoreward-marching breaking surf crest
    let surf_phase = sin(shore_depth * 40.0 - t * 3.25 + (dnx + dnz) * 4.5);
    let surf_wave = smoothstep(0.32, 0.86, 0.5 + 0.5 * surf_phase);
    let outer_surf = (1.0 - smoothstep(0.06, 0.33, shore_depth))
        * smoothstep(0.025, 0.095, shore_depth)
        * surf_wave
        * foam_lace
        * water_params.foam_intensity
        * 0.88;

    // Coastal cliff surge, bridge-pier eddies & concentric naval-hull lapping foam rings
    let pier_ripple = 0.56 + 0.44 * sin(pier_cliff_mask * 22.0 - t * 3.5 + (dnx - dnz) * 4.0);
    let pier_foam = pier_cliff_mask * pier_ripple * foam_lace * water_params.foam_intensity * 1.05;

    let hull_ripple = 0.58 + 0.42 * sin(naval_hull_mask * 24.0 - t * 3.8 + crest_h * 8.0);
    let hull_foam = naval_hull_mask * hull_ripple * foam_lace * water_params.foam_intensity * 1.18;

    let whitecaps = smoothstep(0.14, 0.24, crest_h) * foam_lace * 0.34 * depth_t;
    let foam = clamp(contact_foam + outer_surf + pier_foam + hull_foam + whitecaps, 0.0, 1.0);

    let view_dir = normalize(pbr_input.V);
    let ndotv = clamp(dot(wave_normal, view_dir), 0.0, 1.0);
    let fresnel = pow(1.0 - ndotv, 4.0);

    // Crisp diamond sun specular glints aligned with the NW-to-SE RA2 sun direction
    let sun_dir = normalize(vec3<f32>(-0.64, 0.66, 0.39));
    let half_sun = normalize(sun_dir + view_dir);
    let sun_glint = pow(max(dot(wave_normal, half_sun), 0.0), 185.0) * 1.35 * (1.0 - foam * 0.85);

    water_rgb = mix(water_rgb, vec3<f32>(0.48, 0.80, 0.99), fresnel * 0.42);
    water_rgb = mix(water_rgb, vec3<f32>(0.96, 0.99, 1.0), foam * 0.93);

    // Direct GPU World-XZ Fog-of-War darkening on water
    let fow_shroud = clamp(fow_rgba.a * water_params.fow_params.x, 0.0, 0.97);
    let explored_w = smoothstep(0.05, 0.62, fow_shroud);
    let unexplored_w = smoothstep(0.60, 0.94, fow_shroud);
    water_rgb = mix(water_rgb, water_rgb * vec3<f32>(0.34, 0.42, 0.56), explored_w * 0.78);
    water_rgb = mix(water_rgb, vec3<f32>(0.008, 0.015, 0.030), unexplored_w * 0.96);

    let base_alpha = clamp(mix(0.68, 0.95, depth_t) + fresnel * 0.14 + foam * 0.25, 0.0, 0.99);
    let alpha = mix(base_alpha * edge_feather, max(base_alpha * edge_feather, unexplored_w * 0.96), unexplored_w);

    pbr_input.material.base_color = vec4<f32>(water_rgb, alpha);
    pbr_input.material.perceptual_roughness = mix(mix(0.045, 0.42, foam), 0.92, unexplored_w);
    pbr_input.material.metallic = 0.14 * (1.0 - foam) * (1.0 - unexplored_w);
    pbr_input.material.reflectance = mix(0.96, 0.08, unexplored_w);
    pbr_input.material.emissive = vec4<f32>(
        vec3<f32>(1.0, 0.97, 0.90) * sun_glint * (1.0 - explored_w * 0.85) * (1.0 - unexplored_w),
        0.0,
    );
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
