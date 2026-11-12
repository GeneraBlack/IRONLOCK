use crate::camera::RtsCameraRig;
use crate::input::ClientInteractionState;
use crate::rendering::{BuildingPointLight, MapEnvironmentEntity, PreviewStageEntity};
use crate::{ModResource, SimWorldResource};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use rts_simulation::pathfinding::CollisionGrid;
use rts_simulation::world::EnvironmentPreset;

/// Active atmospheric weather mode resolved from `MapDef.initial_weather` and `EnvironmentPreset` (`Task 206`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ActiveWeatherKind {
    #[default]
    Clear,
    Rain,
    Snow,
    Sandstorm,
    IonStorm,
}

/// Resolves the live weather state from `MapDef.initial_weather` (`"rain"`, `"snow"`, `"sandstorm"`,
/// `"ion_storm"`, `"clear"`) combined with runtime `EnvironmentPreset` (`EnvironmentPreset::IonStorm`).
pub fn resolve_active_weather(
    initial_weather: Option<&str>,
    interaction_preset: EnvironmentPreset,
    world_preset: EnvironmentPreset,
) -> ActiveWeatherKind {
    if interaction_preset == EnvironmentPreset::IonStorm
        || world_preset == EnvironmentPreset::IonStorm
    {
        return ActiveWeatherKind::IonStorm;
    }
    let Some(raw) = initial_weather else {
        return ActiveWeatherKind::Clear;
    };
    let low = raw.trim().to_ascii_lowercase();
    if low.is_empty() || low == "clear" || low == "sunny" || low == "day" || low == "none" {
        ActiveWeatherKind::Clear
    } else if low.contains("ion") || low.contains("lightning") || low.contains("thunder") {
        ActiveWeatherKind::IonStorm
    } else if low.contains("sand") || low.contains("dust") || low.contains("desert") {
        ActiveWeatherKind::Sandstorm
    } else if low.contains("snow") || low.contains("blizzard") || low.contains("winter") {
        ActiveWeatherKind::Snow
    } else if low.contains("rain") || low.contains("storm") || low.contains("monsoon") {
        ActiveWeatherKind::Rain
    } else {
        ActiveWeatherKind::Clear
    }
}

/// Marker component for the single GPU-batched atmospheric weather & Ion-Storm lightning mesh (`Task 206`).
#[derive(Component)]
pub struct WeatherVfxBatchEntity;

/// Dedicated point-light flash entity illuminated during `IonStorm` sky-to-ground lightning bolts.
#[derive(Component)]
pub struct WeatherIonLightningLightEntity;

#[derive(Resource, Default)]
pub struct WeatherVfxState {
    pub initialized: bool,
    pub weather_mesh: Handle<Mesh>,
    pub weather_mat: Handle<StandardMaterial>,
    pub active_weather: ActiveWeatherKind,
    pub bolt_timer_secs: f32,
    pub active_bolt_age: f32,
    pub active_bolt_lifetime: f32,
    pub active_bolt_ground_pos: Vec3,
    pub active_bolt_seed: u32,
}

#[derive(Default)]
pub struct WeatherGeometryBuilder {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

impl WeatherGeometryBuilder {
    #[inline]
    fn push_quad(&mut self, p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, rgba: [f32; 4]) {
        let base = self.positions.len() as u32;
        self.positions.extend_from_slice(&[
            p0.to_array(),
            p1.to_array(),
            p2.to_array(),
            p3.to_array(),
        ]);
        self.normals.extend_from_slice(&[[0.0, 1.0, 0.0]; 4]);
        self.colors.extend_from_slice(&[rgba; 4]);
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Pushes a camera-facing streak ribbon from `a` to `b` with half-width `half_w`.
    #[inline]
    pub fn push_streak(
        &mut self,
        a: Vec3,
        b: Vec3,
        cam_right: Vec3,
        half_w: f32,
        rgba_top: [f32; 4],
        rgba_bot: [f32; 4],
    ) {
        let dir = (b - a).normalize_or_zero();
        let mut side = dir.cross(cam_right.cross(Vec3::Y)).normalize_or_zero();
        if side.length_squared() < 0.01 {
            side = cam_right;
        }
        let r = side * half_w;
        let base = self.positions.len() as u32;
        self.positions.extend_from_slice(&[
            (a - r).to_array(),
            (a + r).to_array(),
            (b + r).to_array(),
            (b - r).to_array(),
        ]);
        self.normals.extend_from_slice(&[[0.0, 1.0, 0.0]; 4]);
        self.colors
            .extend_from_slice(&[rgba_top, rgba_top, rgba_bot, rgba_bot]);
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    pub fn push_ground_ring(&mut self, center: Vec3, radius: f32, half_w: f32, rgba: [f32; 4]) {
        let segs = 12usize;
        let r_in = (radius - half_w).max(0.02);
        let r_out = radius + half_w;
        for i in 0..segs {
            let a0 = (i as f32 / segs as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32 / segs as f32) * std::f32::consts::TAU;
            let (s0, c0) = a0.sin_cos();
            let (s1, c1) = a1.sin_cos();
            self.push_quad(
                Vec3::new(center.x + c0 * r_in, center.y, center.z + s0 * r_in),
                Vec3::new(center.x + c0 * r_out, center.y, center.z + s0 * r_out),
                Vec3::new(center.x + c1 * r_out, center.y, center.z + s1 * r_out),
                Vec3::new(center.x + c1 * r_in, center.y, center.z + s1 * r_in),
                rgba,
            );
        }
    }

    /// Moves all built vertex/index buffers directly into `mesh` with ZERO `.clone()` heap copies (`Task 205b`).
    pub fn write_into_mesh(&mut self, mesh: &mut Mesh) {
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            std::mem::take(&mut self.positions),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, std::mem::take(&mut self.normals));
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, std::mem::take(&mut self.colors));
        mesh.insert_indices(Indices::U32(std::mem::take(&mut self.indices)));
    }
}

#[inline]
fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

#[inline]
fn hash_f32_01(seed: u32) -> f32 {
    (hash_u32(seed) & 0xffff) as f32 / 65535.0
}

fn new_empty_weather_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, Vec::<[f32; 3]>::new());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, Vec::<[f32; 4]>::new());
    mesh.insert_indices(Indices::U32(Vec::new()));
    mesh
}

/// Builds camera-centered atmospheric weather geometry (`Rain`, `Snow`, `Sandstorm`, `IonStorm` + jagged 3D lightning bolts).
#[allow(clippy::too_many_arguments)]
pub fn build_weather_geometry(
    kind: ActiveWeatherKind,
    elapsed: f32,
    focus: Vec3,
    cam_right: Vec3,
    cam_up: Vec3,
    grid: Option<&CollisionGrid>,
    bolt_opt: Option<(Vec3, u32, f32)>,
    out: &mut WeatherGeometryBuilder,
) {
    if kind == ActiveWeatherKind::Clear {
        return;
    }

    let span_xz = 64.0_f32;
    let half_span = span_xz * 0.5;
    let box_h = 24.0_f32;

    let sample_ground = |wx: f32, wz: f32| -> f32 {
        grid.map(|g| g.sample_world_height(wx, wz))
            .unwrap_or(0.0)
            .max(0.02)
    };

    match kind {
        ActiveWeatherKind::Clear => {}
        ActiveWeatherKind::Rain | ActiveWeatherKind::IonStorm => {
            let is_ion = kind == ActiveWeatherKind::IonStorm;
            let count = if is_ion { 180u32 } else { 220u32 };
            let fall_speed = if is_ion { 25.0 } else { 22.0 };
            let wind = if is_ion {
                Vec3::new(4.2, -fall_speed, 2.4)
            } else {
                Vec3::new(2.8, -fall_speed, 1.6)
            };
            let streak_dt = 0.052_f32;
            let base_col = if is_ion {
                [0.64, 0.78, 1.0, 0.52]
            } else {
                [0.72, 0.84, 0.98, 0.46]
            };

            for i in 0..count {
                let rx = hash_f32_01(i * 17 + 3) * span_xz;
                let rz = hash_f32_01(i * 31 + 11) * span_xz;
                let phase = hash_f32_01(i * 47 + 29);

                let cyc = (phase + elapsed * (fall_speed / box_h)).fract();
                let wx = focus.x - half_span + (rx + elapsed * wind.x).rem_euclid(span_xz);
                let wz = focus.z - half_span + (rz + elapsed * wind.z).rem_euclid(span_xz);
                let gy = sample_ground(wx, wz);
                let wy = gy + (1.0 - cyc) * box_h;

                let top = Vec3::new(wx, wy, wz);
                let bot = top + wind * streak_dt;
                out.push_streak(
                    top,
                    bot,
                    cam_right,
                    0.024,
                    [base_col[0], base_col[1], base_col[2], 0.08],
                    base_col,
                );

                // Expanding raindrop ground splash ring when drop reaches surface
                if cyc > 0.88 && (i % 5 == 0) {
                    let splash_p = ((cyc - 0.88) / 0.12).clamp(0.0, 1.0);
                    out.push_ground_ring(
                        Vec3::new(wx, gy + 0.05, wz),
                        0.12 + splash_p * 0.42,
                        0.025,
                        [0.78, 0.90, 1.0, (1.0 - splash_p) * 0.45],
                    );
                }
            }

            // Jagged branching 3D sky-to-ground Ion Lightning Bolt + ground plasma shock-ring
            if let Some((strike_pos, seed, bolt_norm_age)) = bolt_opt {
                let alpha = (1.0 - bolt_norm_age).clamp(0.0, 1.0).powf(0.65);
                let sky_top = strike_pos
                    + Vec3::new(
                        (hash_f32_01(seed + 1) - 0.5) * 5.5,
                        26.0,
                        (hash_f32_01(seed + 2) - 0.5) * 5.5,
                    );
                let segments = 12u32;
                let mut prev = sky_top;
                for s in 1..=segments {
                    let t = s as f32 / segments as f32;
                    let jitter_amp = (1.0 - t) * 1.45 * t.sqrt();
                    let jx = (hash_f32_01(seed.wrapping_add(s * 73)) - 0.5) * 2.0 * jitter_amp;
                    let jz = (hash_f32_01(seed.wrapping_add(s * 139)) - 0.5) * 2.0 * jitter_amp;
                    let curr = if s == segments {
                        strike_pos
                    } else {
                        sky_top.lerp(strike_pos, t) + Vec3::new(jx, 0.0, jz)
                    };
                    // Outer electric violet/cyan plasma sheath + bright white-hot core
                    out.push_streak(
                        prev,
                        curr,
                        cam_right,
                        0.26,
                        [0.48, 0.72, 1.0, alpha * 0.52],
                        [0.68, 0.45, 1.0, alpha * 0.58],
                    );
                    out.push_streak(
                        prev,
                        curr,
                        cam_right,
                        0.085,
                        [0.96, 0.99, 1.0, alpha],
                        [0.88, 0.96, 1.0, alpha],
                    );

                    // Forked secondary lightning branch mid-air
                    if (s == 4 || s == 7) && alpha > 0.2 {
                        let branch_dir = Vec3::new(
                            (hash_f32_01(seed.wrapping_add(s * 311)) - 0.5) * 5.2,
                            -4.4,
                            (hash_f32_01(seed.wrapping_add(s * 419)) - 0.5) * 5.2,
                        );
                        let mid = curr + branch_dir * 0.52 + Vec3::new(0.45, 0.0, -0.35);
                        let tip = curr + branch_dir;
                        out.push_streak(
                            curr,
                            mid,
                            cam_right,
                            0.06,
                            [0.78, 0.88, 1.0, alpha * 0.85],
                            [0.58, 0.48, 1.0, alpha * 0.55],
                        );
                        out.push_streak(
                            mid,
                            tip,
                            cam_right,
                            0.035,
                            [0.58, 0.48, 1.0, alpha * 0.55],
                            [0.48, 0.32, 1.0, 0.0],
                        );
                    }
                    prev = curr;
                }

                out.push_ground_ring(
                    strike_pos + Vec3::Y * 0.08,
                    0.8 + bolt_norm_age * 5.6,
                    0.18,
                    [0.56, 0.84, 1.0, alpha * 0.88],
                );
            }
        }
        ActiveWeatherKind::Snow => {
            let count = 180u32;
            let fall_speed = 3.4_f32;
            let r = cam_right.normalize_or_zero();
            let u = cam_up.normalize_or_zero();
            for i in 0..count {
                let rx = hash_f32_01(i * 19 + 5) * span_xz;
                let rz = hash_f32_01(i * 37 + 13) * span_xz;
                let phase = hash_f32_01(i * 53 + 7);
                let cyc = (phase + elapsed * (fall_speed / box_h)).fract();
                let flutter_x = (elapsed * 1.6 + i as f32 * 0.7).sin() * 0.85;
                let flutter_z = (elapsed * 1.3 + i as f32 * 1.1).cos() * 0.85;
                let wx = focus.x - half_span + (rx + elapsed * 0.9 + flutter_x).rem_euclid(span_xz);
                let wz = focus.z - half_span + (rz + elapsed * 0.5 + flutter_z).rem_euclid(span_xz);
                let gy = sample_ground(wx, wz);
                let wy = gy + (1.0 - cyc) * box_h;
                let c = Vec3::new(wx, wy, wz);
                let sz = 0.065 + hash_f32_01(i * 71) * 0.055;
                let dx = r * sz;
                let dy = u * sz;
                out.push_quad(
                    c - dx - dy,
                    c + dx - dy,
                    c + dx + dy,
                    c - dx + dy,
                    [0.94, 0.97, 1.0, 0.78],
                );
            }
        }
        ActiveWeatherKind::Sandstorm => {
            let count = 140u32;
            let wind = Vec3::new(18.5, -1.2, 6.2);
            for i in 0..count {
                let rx = hash_f32_01(i * 23 + 9) * span_xz;
                let rz = hash_f32_01(i * 41 + 17) * span_xz;
                let h_off = hash_f32_01(i * 61 + 3).powf(1.4) * 11.5 + 0.25;
                let wx = focus.x - half_span + (rx + elapsed * wind.x).rem_euclid(span_xz);
                let wz = focus.z - half_span + (rz + elapsed * wind.z).rem_euclid(span_xz);
                let gy = sample_ground(wx, wz);
                let wy = gy + h_off + (elapsed * 2.4 + i as f32).sin() * 0.35;
                let a = Vec3::new(wx, wy, wz);
                let b = a + wind.normalize_or_zero() * (1.8 + hash_f32_01(i * 89) * 1.9);
                let alpha =
                    (0.28 + (1.0 - (h_off / 12.0).clamp(0.0, 1.0)) * 0.28).clamp(0.12, 0.58);
                out.push_streak(
                    a,
                    b,
                    cam_right,
                    0.16 + hash_f32_01(i * 97) * 0.18,
                    [0.88, 0.68, 0.38, 0.05],
                    [0.86, 0.64, 0.34, alpha],
                );
            }
        }
    }
}

/// Renders camera-centered GPU-batched `Rain`, `Snow`, `Sandstorm`, and `IonStorm` 3D lightning bolts (`Task 206`).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sync_weather_vfx(
    mut commands: Commands,
    time: Res<Time>,
    mod_res: Option<Res<ModResource>>,
    sim_res: Option<Res<SimWorldResource>>,
    interaction: Option<Res<ClientInteractionState>>,
    mut state: ResMut<WeatherVfxState>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
    batch_q: Query<Entity, With<WeatherVfxBatchEntity>>,
    mut flash_q: Query<
        (&mut PointLight, &mut Transform),
        (
            With<WeatherIonLightningLightEntity>,
            Without<RtsCameraRig>,
            Without<BuildingPointLight>,
            Without<PreviewStageEntity>,
        ),
    >,
    camera_q: Query<
        (&RtsCameraRig, &Transform),
        (
            Without<WeatherIonLightningLightEntity>,
            Without<BuildingPointLight>,
            Without<PreviewStageEntity>,
        ),
    >,
) {
    let (Some(mod_res), Some(sim_res), Some(mut meshes), Some(mut materials)) =
        (mod_res, sim_res, meshes, materials)
    else {
        return;
    };

    if !state.initialized {
        state.weather_mesh = meshes.add(new_empty_weather_mesh());
        state.weather_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive: LinearRgba::new(1.45, 1.45, 1.55, 1.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        });
        state.bolt_timer_secs = 1.6;
        state.initialized = true;
    }

    if batch_q.is_empty() {
        commands.spawn((
            MapEnvironmentEntity,
            WeatherVfxBatchEntity,
            Mesh3d(state.weather_mesh.clone()),
            MeshMaterial3d(state.weather_mat.clone()),
            bevy::pbr::NotShadowCaster,
            bevy::pbr::NotShadowReceiver,
            bevy::render::view::NoFrustumCulling,
            Transform::IDENTITY,
        ));
    }
    if flash_q.is_empty() {
        commands.spawn((
            MapEnvironmentEntity,
            WeatherIonLightningLightEntity,
            PointLight {
                color: Color::srgb(0.62, 0.82, 1.0),
                intensity: 0.0,
                range: 42.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(0.0, 12.0, 0.0),
        ));
    }

    let world = &sim_res.0;
    let map = &mod_res.0.entry_map;
    let inter_preset = interaction
        .as_deref()
        .map(|i| i.environment_preset)
        .unwrap_or(world.environment_preset);
    let weather = resolve_active_weather(
        map.initial_weather.as_deref(),
        inter_preset,
        world.environment_preset,
    );
    let prev_weather = state.active_weather;
    state.active_weather = weather;

    if weather == ActiveWeatherKind::Clear {
        if prev_weather != ActiveWeatherKind::Clear {
            if let Some(mesh) = meshes.get_mut(&state.weather_mesh) {
                let mut empty = WeatherGeometryBuilder::default();
                empty.write_into_mesh(mesh);
            }
            for (mut pl, _) in &mut flash_q {
                pl.intensity = 0.0;
            }
        }
        return;
    }

    let dt = time.delta_secs().clamp(0.0, 0.1);
    let elapsed = time.elapsed_secs();

    let (focus, cam_right, cam_up) = if let Ok((rig, cam_tf)) = camera_q.get_single() {
        (rig.focus, cam_tf.right().as_vec3(), cam_tf.up().as_vec3())
    } else {
        let hw = (world.grid.width.max(1) as f32 * world.grid.tile_size) * 0.5;
        let hh = (world.grid.height.max(1) as f32 * world.grid.tile_size) * 0.5;
        (
            Vec3::new(hw, 0.0, hh),
            Vec3::X,
            Vec3::new(0.0, 0.82, -0.57).normalize(),
        )
    };

    // Drive IonStorm lightning strike scheduler & dynamic PointLight flash
    let mut bolt_opt: Option<(Vec3, u32, f32)> = None;
    if weather == ActiveWeatherKind::IonStorm {
        if state.active_bolt_lifetime > 0.0 {
            state.active_bolt_age += dt;
            if state.active_bolt_age >= state.active_bolt_lifetime {
                state.active_bolt_lifetime = 0.0;
            } else {
                let norm = (state.active_bolt_age / state.active_bolt_lifetime).clamp(0.0, 1.0);
                bolt_opt = Some((state.active_bolt_ground_pos, state.active_bolt_seed, norm));
            }
        } else {
            state.bolt_timer_secs -= dt;
            if state.bolt_timer_secs <= 0.0 {
                let seed = hash_u32((elapsed * 1_000.0) as u32 ^ (world.tick as u32));
                let ox = (hash_f32_01(seed.wrapping_add(11)) - 0.5) * 34.0;
                let oz = (hash_f32_01(seed.wrapping_add(29)) - 0.5) * 34.0;
                let sx = (focus.x + ox).clamp(
                    1.0,
                    (world.grid.width.max(2) as f32 * world.grid.tile_size) - 1.0,
                );
                let sz = (focus.z + oz).clamp(
                    1.0,
                    (world.grid.height.max(2) as f32 * world.grid.tile_size) - 1.0,
                );
                let sy = world.grid.sample_world_height(sx, sz).max(0.04);
                state.active_bolt_ground_pos = Vec3::new(sx, sy, sz);
                state.active_bolt_seed = seed;
                state.active_bolt_age = 0.0;
                state.active_bolt_lifetime = 0.34;
                state.bolt_timer_secs = 2.1 + hash_f32_01(seed.wrapping_add(53)) * 2.8;
                bolt_opt = Some((state.active_bolt_ground_pos, seed, 0.0));
            }
        }
    } else {
        state.active_bolt_lifetime = 0.0;
    }

    for (mut pl, mut ltf) in &mut flash_q {
        if let Some((bpos, _, norm)) = bolt_opt {
            ltf.translation = bpos + Vec3::Y * 7.5;
            pl.intensity = (1.0 - norm).powi(2) * 340_000.0;
        } else if pl.intensity > 0.0 {
            pl.intensity = 0.0;
        }
    }

    let mut builder = WeatherGeometryBuilder::default();
    build_weather_geometry(
        weather,
        elapsed,
        focus,
        cam_right,
        cam_up,
        Some(&world.grid),
        bolt_opt,
        &mut builder,
    );
    if let Some(mesh) = meshes.get_mut(&state.weather_mesh) {
        builder.write_into_mesh(mesh);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_weather_resolver_and_geometry_builder_cover_rain_snow_sandstorm_and_ion_storm() {
        assert_eq!(
            resolve_active_weather(None, EnvironmentPreset::Day, EnvironmentPreset::Day),
            ActiveWeatherKind::Clear
        );
        assert_eq!(
            resolve_active_weather(Some("rain"), EnvironmentPreset::Day, EnvironmentPreset::Day),
            ActiveWeatherKind::Rain
        );
        assert_eq!(
            resolve_active_weather(Some("snow"), EnvironmentPreset::Day, EnvironmentPreset::Day),
            ActiveWeatherKind::Snow
        );
        assert_eq!(
            resolve_active_weather(
                Some("sandstorm"),
                EnvironmentPreset::Day,
                EnvironmentPreset::Day
            ),
            ActiveWeatherKind::Sandstorm
        );
        assert_eq!(
            resolve_active_weather(None, EnvironmentPreset::IonStorm, EnvironmentPreset::Day),
            ActiveWeatherKind::IonStorm
        );

        let grid = CollisionGrid::new(32, 32, 2.0);
        for mode in [
            ActiveWeatherKind::Rain,
            ActiveWeatherKind::Snow,
            ActiveWeatherKind::Sandstorm,
            ActiveWeatherKind::IonStorm,
        ] {
            let mut b = WeatherGeometryBuilder::default();
            let bolt = (mode == ActiveWeatherKind::IonStorm).then_some((
                Vec3::new(16.0, 0.0, 16.0),
                42u32,
                0.15,
            ));
            build_weather_geometry(
                mode,
                1.25,
                Vec3::new(16.0, 0.0, 16.0),
                Vec3::X,
                Vec3::Y,
                Some(&grid),
                bolt,
                &mut b,
            );
            assert!(
                b.positions.len() >= 400,
                "{mode:?} must emit batched atmospheric particle geometry"
            );
            let mut mesh = new_empty_weather_mesh();
            b.write_into_mesh(&mut mesh);
            assert!(mesh.count_vertices() >= 400);
            assert!(
                b.positions.is_empty(),
                "write_into_mesh must move buffers without cloning"
            );
        }
    }
}
