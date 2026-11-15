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

struct TeamDamageUniform {
    // rgb = accessible team color, a = team_tint_strength (0.0 for Disabled / slot 0 subtle)
    team_color: vec4<f32>,
    // x = metallic_override (< 0 means keep glTF), y = roughness_override (< 0 means keep glTF),
    // z = emissive_boost, w = team_mask_threshold
    pbr_overrides: vec4<f32>,
    // x = damage_soot (0.0 = Intact, 0.48 = Yellow50, 0.88 = Red25),
    // y = shield_fresnel_strength (0.0..1.0),
    // z = mask_mode (0=AutoNeutralGray, 1=AlphaMask, 2=VertexColorMask, 3=FullTint, 4=Disabled),
    // w = strategic_fade_alpha (0.0..1.0)
    damage_and_fx: vec4<f32>,
    // rgb = shield / chrono / aura Fresnel rim glow color, a = pulse speed
    shield_color: vec4<f32>,
    // xy = map_world_size, z = fow_enabled (0.0 on Studio PreviewStage), w = elapsed_secs
    map_fow_params: vec4<f32>,
}

@group(2) @binding(100) var<uniform> team_ext: TeamDamageUniform;
@group(2) @binding(101) var fow_tex: texture_2d<f32>;
@group(2) @binding(102) var fow_sampler: sampler;

fn soot_hash31(p: vec3<f32>) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + vec3<f32>(33.33));
    return fract((p3.x + p3.y) * p3.z);
}

fn soot_noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (vec3<f32>(3.0) - 2.0 * f);
    let n000 = soot_hash31(i + vec3<f32>(0.0, 0.0, 0.0));
    let n100 = soot_hash31(i + vec3<f32>(1.0, 0.0, 0.0));
    let n010 = soot_hash31(i + vec3<f32>(0.0, 1.0, 0.0));
    let n110 = soot_hash31(i + vec3<f32>(1.0, 1.0, 0.0));
    let n001 = soot_hash31(i + vec3<f32>(0.0, 0.0, 1.0));
    let n101 = soot_hash31(i + vec3<f32>(1.0, 0.0, 1.0));
    let n011 = soot_hash31(i + vec3<f32>(0.0, 1.0, 1.0));
    let n111 = soot_hash31(i + vec3<f32>(1.0, 1.0, 1.0));
    let nx00 = mix(n000, n100, u.x);
    let nx10 = mix(n010, n110, u.x);
    let nx01 = mix(n001, n101, u.x);
    let nx11 = mix(n011, n111, u.x);
    let nxy0 = mix(nx00, nx10, u.y);
    let nxy1 = mix(nx01, nx11, u.y);
    return mix(nxy0, nxy1, u.z);
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    // Preserves 100% of glTF PBR maps (`base_color_texture`, `normal_map_texture`,
    // `metallic_roughness_texture`, `emissive_texture`) AND `COLOR_0` vertex colors!
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    let world_pos = in.world_position.xyz;
    let safe_size = max(team_ext.map_fow_params.xy, vec2<f32>(1.0, 1.0));
    let map_uv = clamp(world_pos.xz / safe_size, vec2<f32>(0.001, 0.001), vec2<f32>(0.999, 0.999));
    let fow_rgba = textureSample(fow_tex, fow_sampler, map_uv);

    var base_rgba = pbr_input.material.base_color;
    var albedo = base_rgba.rgb;
    let max_c = max(max(albedo.r, albedo.g), albedo.b);
    let min_c = min(min(albedo.r, albedo.g), albedo.b);
    let chroma = max_c - min_c;
    let sat = chroma / max(max_c, 0.001);
    let luma = dot(albedo, vec3<f32>(0.299, 0.587, 0.114));

    var vert_col = vec4<f32>(1.0, 1.0, 1.0, 1.0);
#ifdef VERTEX_COLORS
    vert_col = in.color;
#endif

    let threshold = clamp(team_ext.pbr_overrides.w, 0.05, 0.95);
    let mode = i32(round(team_ext.damage_and_fx.z));
    let tint_strength = clamp(team_ext.team_color.a, 0.0, 1.0);
    let team_rgb = team_ext.team_color.rgb;

    var mask_w = 0.0;
    if mode == 0 {
        if base_rgba.a < 0.98 {
            // Explicit Westwood 16..=31 house-color mask packed in base_color.a (`1.0 - a` => team weight)
            mask_w = clamp(1.0 - base_rgba.a, 0.0, 1.0) * tint_strength;
            base_rgba.a = 1.0;
        } else {
            // Tightened AutoNeutralGray fallback ONLY when `base_rgba.a >= 0.98`: preserves dark steel,
            // gun barrels, treads (`luma < 0.34`), specular glass (`luma > 0.92`), and zero floor (`0.0`).
            let neutral_w = 1.0 - smoothstep(threshold * 0.45, threshold + 0.06, sat);
            let mid_luma_w = smoothstep(0.34, 0.50, luma) * (1.0 - smoothstep(0.84, 0.93, luma));
            mask_w = clamp(neutral_w * mid_luma_w * 0.76, 0.0, 1.0) * tint_strength;
        }
    } else if mode == 1 {
        // Exact Westwood AlphaMask contract: `base_rgba.a == 1.0` => 0% team color (steel/treads/concrete/glass);
        // `base_rgba.a < 1.0` => explicit house-color mask weight `1.0 - base_rgba.a` (`a = 0.0` => 100% team).
        mask_w = clamp(1.0 - base_rgba.a, 0.0, 1.0) * tint_strength;
        base_rgba.a = 1.0;
    } else if mode == 2 {
        // VertexColorMask / LuminanceTint: uses `COLOR_0` saturation / mask weight
        let vc_max = max(max(vert_col.r, vert_col.g), vert_col.b);
        let vc_min = min(min(vert_col.r, vert_col.g), vert_col.b);
        let vc_sat = (vc_max - vc_min) / max(vc_max, 0.001);
        let lum_band = smoothstep(threshold * 0.35, threshold + 0.25, luma);
        mask_w = clamp(max(vc_sat, lum_band * 0.72), 0.0, 1.0) * tint_strength;
    } else if mode == 3 {
        // FullTint / EmissiveStripe
        mask_w = 0.82 * tint_strength;
    } else {
        // Disabled
        mask_w = 0.0;
    }

    // Contrast-preserving S-curve transfer on neutral grayscale `base_rgba.rgb` (`luma`):
    // - Dark Cycles AO crevices (`luma ~ 0.15..0.30`) stay genuinely dark (`team_rgb * 0.14..0.32`) with zero floor lift.
    // - Mid-tone armor (`luma ~ 0.60..0.72`) hits vibrant faction color (`team_rgb * 0.96..1.08`).
    // - Bright beveled edges (`luma > 0.80`) mix subtly toward sunlit desaturated specular highlight.
    let mid_s = smoothstep(0.25, 0.62, luma);
    let hi_s = smoothstep(0.58, 0.88, luma);
    let shade_gain = mix(luma * 0.95, 0.96 + 0.26 * hi_s, mid_s);
    let spec_edge = smoothstep(0.80, 0.98, luma) * 0.28;
    let shaded_team = mix(team_rgb * shade_gain, vec3<f32>(1.0), spec_edge);
    albedo = mix(albedo, shaded_team, mask_w);

    // Optional VisualDef PBR overrides (`metallic`, `roughness`, `emissive_boost`).
    // When `< 0.0` (the default for glTF PBR assets), per-pixel `metallicRoughnessTexture`
    // values from `pbr_input_from_standard_material` remain 100% intact.
    if team_ext.pbr_overrides.x >= 0.0 {
        pbr_input.material.metallic = clamp(team_ext.pbr_overrides.x, 0.0, 1.0);
    }
    if team_ext.pbr_overrides.y >= 0.0 {
        pbr_input.material.perceptual_roughness = clamp(team_ext.pbr_overrides.y, 0.04, 1.0);
    }
    var extra_emissive = vec3<f32>(0.0);
    if mode == 3 && tint_strength > 0.05 {
        let stripe = smoothstep(0.72, 0.92, abs(sin(world_pos.x * 2.4 + world_pos.z * 2.4)));
        extra_emissive = extra_emissive + team_rgb * (stripe * 0.35 * max(team_ext.pbr_overrides.z, 0.35));
    }

    // Fragment-shader procedural soot/char damage darkening + glowing Red25 ember fissures (`Task 236`)
    // Capped at `0.52` max so damaged hulls retain readable faction livery instead of over-blackening.
    let damage_soot = clamp(team_ext.damage_and_fx.x, 0.0, 1.0);
    if damage_soot > 0.01 {
        let char_n = soot_noise3(world_pos * 2.6) * 0.65 + soot_noise3(world_pos * 6.8) * 0.35;
        let soot_patch = clamp(damage_soot * (0.36 + char_n * 0.44), 0.0, 0.52);
        albedo = mix(albedo, albedo * vec3<f32>(0.26, 0.22, 0.19), soot_patch);
        pbr_input.material.perceptual_roughness = mix(
            pbr_input.material.perceptual_roughness,
            0.92,
            soot_patch * 0.75,
        );
        pbr_input.material.metallic = pbr_input.material.metallic * (1.0 - soot_patch * 0.55);

        if damage_soot > 0.65 {
            let ember_fissure = smoothstep(0.70, 0.90, char_n) * ((damage_soot - 0.65) / 0.35);
            let pulse = 0.75 + 0.25 * sin(team_ext.map_fow_params.w * 6.5 + world_pos.x * 3.1);
            extra_emissive = extra_emissive + vec3<f32>(2.4, 0.52, 0.08) * (ember_fissure * pulse);
        }
    }

    // Shield / Iron Curtain / Chrono / Hero Aura Fresnel rim illumination (`Task 236`)
    let shield_strength = clamp(team_ext.damage_and_fx.y, 0.0, 1.0);
    if shield_strength > 0.01 {
        let ndotv = clamp(dot(normalize(pbr_input.N), normalize(pbr_input.V)), 0.0, 1.0);
        let rim = pow(1.0 - ndotv, 2.75);
        let shimmer = 0.82 + 0.18 * sin(team_ext.map_fow_params.w * 5.0 + world_pos.y * 4.0);
        extra_emissive = extra_emissive + team_ext.shield_color.rgb * (rim * shield_strength * 2.4 * shimmer);
        albedo = mix(albedo, team_ext.shield_color.rgb, rim * shield_strength * 0.35);
    }

    // Direct GPU World-XZ Fog-of-War attenuation on explored buildings & terrain-adjacent structures
    let fow_shroud = clamp(fow_rgba.a * team_ext.map_fow_params.z, 0.0, 0.96);
    let explored_w = smoothstep(0.08, 0.64, fow_shroud);
    let unexplored_w = smoothstep(0.62, 0.94, fow_shroud);
    albedo = mix(albedo, albedo * vec3<f32>(0.36, 0.42, 0.56), explored_w * 0.72);
    albedo = mix(albedo, vec3<f32>(0.012, 0.018, 0.032), unexplored_w * 0.92);
    extra_emissive = extra_emissive * (1.0 - explored_w * 0.75);

    let strategic_fade = clamp(team_ext.damage_and_fx.w, 0.0, 1.0);
    if strategic_fade < 0.995 {
        let ign = fract(52.9829189 * fract(dot(in.position.xy, vec2<f32>(0.06711056, 0.00583715))));
        if ign > strategic_fade {
            discard;
        }
    }
    pbr_input.material.base_color = vec4<f32>(albedo, base_rgba.a * strategic_fade);
    pbr_input.material.emissive = vec4<f32>(
        pbr_input.material.emissive.rgb * (1.0 + max(team_ext.pbr_overrides.z, 0.0)) + extra_emissive,
        pbr_input.material.emissive.a,
    );
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
