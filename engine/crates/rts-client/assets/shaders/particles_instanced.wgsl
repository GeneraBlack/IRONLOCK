#import bevy_pbr::{
    forward_io::{Vertex, VertexOutput, FragmentOutput},
    mesh_functions::{get_world_from_local, mesh_position_local_to_world},
    view_transformations::position_world_to_clip,
}

struct GpuParticleUniform {
    // xyz = camera right axis, w = current_time_secs
    camera_right_and_time: vec4<f32>,
    // xyz = camera up axis, w = floor_bounce_y
    camera_up_and_floor: vec4<f32>,
    // rgb = secondary/end tint multiplier, a = global_hdr_boost
    end_tint_and_boost: vec4<f32>,
}

@group(2) @binding(0) var<uniform> particle_params: GpuParticleUniform;
@group(2) @binding(1) var particle_tex: texture_2d<f32>;
@group(2) @binding(2) var particle_sampler: sampler;

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

    let world_from_local = get_world_from_local(vertex.instance_index);
    var origin = vec3<f32>(0.0);
#ifdef VERTEX_POSITIONS
    origin = mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0)).xyz;
#endif

    var init_vel = vec3<f32>(0.0, 1.0, 0.0);
#ifdef VERTEX_NORMALS
    init_vel = vertex.normal;
#endif

    var corner_uv = vec2<f32>(0.5, 0.5);
#ifdef VERTEX_UVS_A
    corner_uv = vertex.uv;
#endif

    // uv_b.x = spawn_time_secs, uv_b.y = lifetime_secs
    var spawn_and_life = vec2<f32>(0.0, 1.0);
#ifdef VERTEX_UVS_B
    spawn_and_life = vertex.uv_b;
#endif

    // tangent = (gravity, drag, start_size, end_size)
    var kin = vec4<f32>(0.0, 0.25, 0.25, 0.05);
#ifdef VERTEX_TANGENTS
    kin = vertex.tangent;
#endif

    var start_col = vec4<f32>(1.0, 1.0, 1.0, 1.0);
#ifdef VERTEX_COLORS
    start_col = vertex.color;
#endif

    let now = particle_params.camera_right_and_time.w;
    let lifetime = max(spawn_and_life.y, 0.001);
    let age = now - spawn_and_life.x;

    // Cull unborn or expired particles directly in the GPU vertex shader (`0` CPU work!)
    if age < -0.001 || age >= lifetime {
        out.position = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        out.world_position = vec4<f32>(origin, 1.0);
        out.world_normal = vec3<f32>(0.0, 1.0, 0.0);
#ifdef VERTEX_UVS_A
        out.uv = corner_uv;
#endif
#ifdef VERTEX_UVS_B
        out.uv_b = vec2<f32>(1.0, 0.0);
#endif
#ifdef VERTEX_TANGENTS
        out.world_tangent = vec4<f32>(1.0, 0.0, 0.0, 1.0);
#endif
#ifdef VERTEX_COLORS
        out.color = vec4<f32>(0.0);
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
        out.instance_index = vertex.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
        out.visibility_range_dither = 0;
#endif
        return out;
    }

    let clamped_age = max(age, 0.0);
    let norm_t = clamp(clamped_age / lifetime, 0.0, 1.0);
    let gravity = kin.x;
    let drag = max(kin.y, 0.0);

    var eff_t = clamped_age;
    if drag > 0.001 {
        eff_t = (1.0 - exp(-drag * clamped_age)) / drag;
    }

    var center = origin + init_vel * eff_t + vec3<f32>(0.0, -0.5 * gravity * clamped_age * clamped_age, 0.0);
    let min_floor = particle_params.camera_up_and_floor.w;
    if gravity > 0.0 && center.y < min_floor {
        let pen = min_floor - center.y;
        center.y = min_floor + min(pen * 0.28, 0.45);
    }

    // Smooth 3-stop size ramp (`start -> mid swell -> end`)
    let s0 = max(kin.z, 0.01);
    let s2 = max(kin.w, 0.01);
    let s1 = mix(s0, s2, 0.45) * 1.18;
    var size = s0;
    if norm_t < 0.5 {
        size = mix(s0, s1, norm_t * 2.0);
    } else {
        size = mix(s1, s2, (norm_t - 0.5) * 2.0);
    }

    let cam_right = normalize(particle_params.camera_right_and_time.xyz);
    let cam_up = normalize(particle_params.camera_up_and_floor.xyz);
    let quad_offset = (corner_uv.x - 0.5) * size * cam_right + (0.5 - corner_uv.y) * size * cam_up;
    let world_pos = center + quad_offset;

    // 3-stop HDR color & alpha ramp (`hot core -> mid glow -> cooled ember/smoke fade`)
    let boost = max(particle_params.end_tint_and_boost.w, 0.1);
    let c0 = start_col.rgb * boost;
    let c1 = mix(c0, c0 * particle_params.end_tint_and_boost.rgb, 0.42);
    let c2 = c0 * particle_params.end_tint_and_boost.rgb * 0.35;
    var ramp_rgb = c0;
    if norm_t < 0.5 {
        ramp_rgb = mix(c0, c1, norm_t * 2.0);
    } else {
        ramp_rgb = mix(c1, c2, (norm_t - 0.5) * 2.0);
    }
    let fade_in = smoothstep(0.0, 0.08, norm_t);
    let fade_out = pow(1.0 - norm_t, 1.15);
    let alpha = clamp(start_col.a * fade_in * fade_out, 0.0, 1.0);

    out.position = position_world_to_clip(world_pos);
    out.world_position = vec4<f32>(world_pos, 1.0);
    out.world_normal = normalize(cross(cam_right, cam_up));
#ifdef VERTEX_UVS_A
    out.uv = corner_uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = vec2<f32>(norm_t, alpha);
#endif
#ifdef VERTEX_TANGENTS
    out.world_tangent = vec4<f32>(cam_right, 1.0);
#endif
#ifdef VERTEX_COLORS
    out.color = vec4<f32>(ramp_rgb, alpha);
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = 0;
#endif
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var uv = vec2<f32>(0.5, 0.5);
#ifdef VERTEX_UVS_A
    uv = in.uv;
#endif
    let mask = textureSample(particle_tex, particle_sampler, uv);

    var col = vec4<f32>(1.0, 1.0, 1.0, 1.0);
#ifdef VERTEX_COLORS
    col = in.color;
#endif

    let out_alpha = col.a * mask.a;
    if out_alpha <= 0.004 {
        discard;
    }

    var out: FragmentOutput;
    out.color = vec4<f32>(col.rgb * mask.rgb, out_alpha);
    return out;
}
