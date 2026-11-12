pub mod prefabs_and_cutscenes;

pub use prefabs_and_cutscenes::{
    capture_selection_as_prefab, rotate_prefab_offset, rotated_prefab_footprint,
    sample_cutscene_camera_pose, stamp_prefab_at_tile,
};

use super::world_builder::ensure_heightmap_and_splatmap;
use rts_sdk::loader::LoadedMod;
use rts_sdk::schema::{MapDef, MapPrefabDef};
use rts_simulation::world::SimWorld;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SplineKind {
    #[default]
    Road,
    River,
    CliffRidge,
}

impl SplineKind {
    pub const ALL: [SplineKind; 3] = [Self::Road, Self::River, Self::CliffRidge];

    pub fn label(self) -> &'static str {
        match self {
            Self::Road => "Road (Asphalt + Flatten)",
            Self::River => "River (Carve Bed + Water)",
            Self::CliffRidge => "Cliff Ridge (Raise + CliffRock + Block)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActiveAdvancedTerrainMode {
    #[default]
    None,
    SplineAuthoring,
    AutoRampGrading,
    PrefabStamp,
}

#[derive(Debug, Clone)]
pub struct SplineAndPrefabToolState {
    pub mode: ActiveAdvancedTerrainMode,
    pub spline_kind: SplineKind,
    pub spline_points: Vec<[f32; 3]>,
    pub dragging_spline_pt: Option<usize>,
    pub spline_half_width_tiles: f32,
    pub spline_height_param: f32,

    // Auto-Ramp Grading
    pub ramp_drag_start_tile: Option<[i32; 2]>,
    pub ramp_drag_end_tile: Option<[i32; 2]>,
    pub ramp_half_width_tiles: f32,

    // Prefab Capture & Stamp Brush
    pub new_prefab_id: String,
    pub new_prefab_name: String,
    pub new_prefab_category: String,
    pub cached_prefabs: Vec<MapPrefabDef>,
    pub prefabs_loaded_for_mod: String,
    pub selected_prefab_idx: usize,
    pub prefab_rotation_quarters: u8,

    // Cutscene Frustum & Preview
    pub selected_cutscene_idx: usize,
    pub selected_keyframe_idx: Option<usize>,
    pub active_cutscene_preview: Option<(String, f32)>,
    pub new_cutscene_id: String,
}

impl Default for SplineAndPrefabToolState {
    fn default() -> Self {
        Self {
            mode: ActiveAdvancedTerrainMode::None,
            spline_kind: SplineKind::Road,
            spline_points: Vec::new(),
            dragging_spline_pt: None,
            spline_half_width_tiles: 1.5,
            spline_height_param: 1.2,
            ramp_drag_start_tile: None,
            ramp_drag_end_tile: None,
            ramp_half_width_tiles: 1.5,
            new_prefab_id: "forward_outpost".to_string(),
            new_prefab_name: "Forward Outpost".to_string(),
            new_prefab_category: "military".to_string(),
            cached_prefabs: Vec::new(),
            prefabs_loaded_for_mod: String::new(),
            selected_prefab_idx: 0,
            prefab_rotation_quarters: 0,
            selected_cutscene_idx: 0,
            selected_keyframe_idx: None,
            active_cutscene_preview: None,
            new_cutscene_id: "intro_flyover".to_string(),
        }
    }
}

impl SplineAndPrefabToolState {
    pub fn ensure_prefabs_cached(&mut self, loaded: &LoadedMod) {
        let key = loaded.root_dir.to_string_lossy().to_string();
        if self.prefabs_loaded_for_mod != key {
            self.cached_prefabs = loaded
                .load_prefab_defs()
                .map(|m| m.into_values().collect())
                .unwrap_or_default();
            self.prefabs_loaded_for_mod = key;
            if self.selected_prefab_idx >= self.cached_prefabs.len() {
                self.selected_prefab_idx = 0;
            }
        }
    }

    pub fn reload_prefabs(&mut self, loaded: &LoadedMod) {
        self.prefabs_loaded_for_mod.clear();
        self.ensure_prefabs_cached(loaded);
    }
}

fn catmull_rom_scalar(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

/// Evaluates a smooth 3D Catmull-Rom curve across `points` (`samples_per_segment` >= 2).
pub fn evaluate_catmull_rom_3d(points: &[[f32; 3]], samples_per_segment: usize) -> Vec<[f32; 3]> {
    if points.len() < 2 {
        return points.to_vec();
    }
    let steps = samples_per_segment.max(2);
    let n = points.len();
    let mut out = Vec::with_capacity((n - 1) * steps + 1);
    for seg in 0..(n - 1) {
        let p0 = points[seg.saturating_sub(1)];
        let p1 = points[seg];
        let p2 = points[seg + 1];
        let p3 = points[(seg + 2).min(n - 1)];
        for s in 0..steps {
            let t = s as f32 / steps as f32;
            out.push([
                catmull_rom_scalar(p0[0], p1[0], p2[0], p3[0], t),
                catmull_rom_scalar(p0[1], p1[1], p2[1], p3[1], t),
                catmull_rom_scalar(p0[2], p1[2], p2[2], p3[2], t),
            ]);
        }
    }
    if let Some(&last) = points.last() {
        out.push(last);
    }
    out
}

/// Blends an RGBA splat vertex weight toward one of the 5 terrain shader layers:
/// `0 = Grass [255,0,0,0]`, `1 = Dirt [0,255,0,0]`, `2 = Sand [0,0,255,0]`,
/// `3 = Asphalt [0,0,0,255]`, `4 = CliffRock [0,0,0,0]` (`w4 = 1 - sum(r,g,b,a)` in `terrain_splat.wgsl`).
pub fn blend_splat_rgba_toward_layer(rgba: &mut [u8; 4], layer_idx: usize, alpha: f32) {
    let a = alpha.clamp(0.0, 1.0);
    let target: [f32; 4] = match layer_idx {
        0 => [255.0, 0.0, 0.0, 0.0],
        1 => [0.0, 255.0, 0.0, 0.0],
        2 => [0.0, 0.0, 255.0, 0.0],
        3 => [0.0, 0.0, 0.0, 255.0],
        _ => [0.0, 0.0, 0.0, 0.0],
    };
    for c in 0..4 {
        let cur = rgba[c] as f32;
        rgba[c] = (cur + (target[c] - cur) * a).round().clamp(0.0, 255.0) as u8;
    }
}

fn closest_point_on_polyline_xz(samples: &[[f32; 3]], wx: f32, wz: f32) -> Option<(f32, f32)> {
    if samples.is_empty() {
        return None;
    }
    if samples.len() == 1 {
        let dx = wx - samples[0][0];
        let dz = wz - samples[0][2];
        return Some(((dx * dx + dz * dz).sqrt(), samples[0][1]));
    }
    let mut best_dist_sq = f32::MAX;
    let mut best_y = samples[0][1];
    for seg in samples.windows(2) {
        let a = seg[0];
        let b = seg[1];
        let abx = b[0] - a[0];
        let abz = b[2] - a[2];
        let len_sq = abx * abx + abz * abz;
        let t = if len_sq > 1e-6 {
            (((wx - a[0]) * abx + (wz - a[2]) * abz) / len_sq).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let px = a[0] + abx * t;
        let pz = a[2] + abz * t;
        let py = a[1] + (b[1] - a[1]) * t;
        let d_sq = (wx - px) * (wx - px) + (wz - pz) * (wz - pz);
        if d_sq < best_dist_sq {
            best_dist_sq = d_sq;
            best_y = py;
        }
    }
    Some((best_dist_sq.sqrt(), best_y))
}

pub(super) fn sync_grid_from_map_heightmap(map: &MapDef, world: &mut SimWorld) {
    world.grid.sync_heightmap_from_map_def(map);
    if let Some(hm) = map.heightmap.as_ref() {
        for tz in 0..(map.height as i32) {
            for tx in 0..(map.width as i32) {
                let h00 = hm.sample_vertex(tx as u32, tz as u32);
                let h10 = hm.sample_vertex((tx + 1) as u32, tz as u32);
                let h01 = hm.sample_vertex(tx as u32, (tz + 1) as u32);
                let h11 = hm.sample_vertex((tx + 1) as u32, (tz + 1) as u32);
                let avg = (h00 + h10 + h01 + h11) * 0.25;
                if avg >= 0.85 {
                    world.grid.elevation_tiles.insert([tx, tz]);
                } else {
                    world.grid.elevation_tiles.remove(&[tx, tz]);
                }
            }
        }
    }
    world.grid.topology_epoch = world.grid.topology_epoch.wrapping_add(1);
}

/// Rasterizes a 3D Catmull-Rom spline (`Road`, `River`, or `CliffRidge`) into `map` heightmap,
/// `(w+1)x(h+1)` vertex splatmap, and `world.grid` passability/water/obstacle sets.
pub fn rasterize_spline_into_map_and_world(
    points: &[[f32; 3]],
    kind: SplineKind,
    half_width_tiles: f32,
    height_param: f32,
    map: &mut MapDef,
    world: &mut SimWorld,
) -> usize {
    if points.len() < 2 {
        return 0;
    }
    ensure_heightmap_and_splatmap(map);
    let samples = evaluate_catmull_rom_3d(points, 16);
    let ts = map.tile_size.max(1.0);
    let core_radius = half_width_tiles.max(0.5) * ts;
    let blend_radius = (half_width_tiles.max(0.5) + 1.0) * ts;

    let vw = map.width + 1;
    let vh = map.height + 1;
    let mut touched_vertices = 0usize;

    if let (Some(hm), Some(sm)) = (map.heightmap.as_mut(), map.splatmap.as_mut()) {
        for vz in 0..vh {
            for vx in 0..vw {
                let wx = vx as f32 * ts;
                let wz = vz as f32 * ts;
                let Some((dist, curve_y)) = closest_point_on_polyline_xz(&samples, wx, wz) else {
                    continue;
                };
                if dist > blend_radius {
                    continue;
                }
                let falloff = if dist <= core_radius {
                    1.0
                } else {
                    (1.0 - (dist - core_radius) / (blend_radius - core_radius).max(0.001))
                        .clamp(0.0, 1.0)
                };
                let idx = (vz * vw + vx) as usize;
                touched_vertices += 1;

                match kind {
                    SplineKind::Road => {
                        if let Some(h) = hm.heights.get_mut(idx) {
                            *h = (*h + (curve_y - *h) * falloff).clamp(-1.5, 6.0);
                        }
                        if let Some(rgba) = sm.weights_rgba.get_mut(idx) {
                            blend_splat_rgba_toward_layer(rgba, 3, falloff);
                        }
                    }
                    SplineKind::River => {
                        let carve_depth = height_param.abs().max(0.6);
                        let target_y = -carve_depth;
                        if let Some(h) = hm.heights.get_mut(idx) {
                            *h = (*h + (target_y - *h) * falloff).clamp(-1.5, 6.0);
                        }
                        if let Some(rgba) = sm.weights_rgba.get_mut(idx) {
                            // Sand/dirt banks & riverbed
                            let layer = if dist <= core_radius { 2 } else { 1 };
                            blend_splat_rgba_toward_layer(rgba, layer, falloff);
                        }
                    }
                    SplineKind::CliffRidge => {
                        let ridge_h = height_param.abs().max(1.4);
                        if let Some(h) = hm.heights.get_mut(idx) {
                            let raised = (curve_y + ridge_h).clamp(0.5, 6.0);
                            *h = (*h + (raised - *h) * falloff).clamp(-1.5, 6.0);
                        }
                        if let Some(rgba) = sm.weights_rgba.get_mut(idx) {
                            // Layer 4 = CliffRock -> [0, 0, 0, 0]
                            blend_splat_rgba_toward_layer(rgba, 4, falloff);
                        }
                    }
                }
            }
        }
    }

    // Stamp logical tiles along corridor (`water_tiles`, `blocked_tiles`, or clear for `Road`)
    for tz in 0..(map.height as i32) {
        for tx in 0..(map.width as i32) {
            let cx = (tx as f32 + 0.5) * ts;
            let cz = (tz as f32 + 0.5) * ts;
            let Some((dist, _)) = closest_point_on_polyline_xz(&samples, cx, cz) else {
                continue;
            };
            if dist <= core_radius {
                let t = [tx, tz];
                match kind {
                    SplineKind::Road => {
                        world.grid.blocked_tiles.remove(&t);
                        world.grid.water_tiles.remove(&t);
                    }
                    SplineKind::River => {
                        world.grid.water_tiles.insert(t);
                        world.grid.blocked_tiles.remove(&t);
                        world.grid.ramp_tiles.remove(&t);
                    }
                    SplineKind::CliffRidge => {
                        world.grid.blocked_tiles.insert(t);
                        world.grid.water_tiles.remove(&t);
                    }
                }
            }
        }
    }

    sync_grid_from_map_heightmap(map, world);
    touched_vertices
}

/// Smoothly grades a traversable ramp corridor from `start_tile` to `end_tile`, interpolating
/// `(w+1)x(h+1)` vertex heights from the start height to the end height and marking `ramp_tiles`.
pub fn apply_auto_ramp_grading(
    start_tile: [i32; 2],
    end_tile: [i32; 2],
    half_width_tiles: f32,
    map: &mut MapDef,
    world: &mut SimWorld,
) -> usize {
    ensure_heightmap_and_splatmap(map);
    let ts = map.tile_size.max(1.0);
    let sx = (start_tile[0] as f32 + 0.5) * ts;
    let sz = (start_tile[1] as f32 + 0.5) * ts;
    let ex = (end_tile[0] as f32 + 0.5) * ts;
    let ez = (end_tile[1] as f32 + 0.5) * ts;

    let dx = ex - sx;
    let dz = ez - sz;
    let seg_len_sq = dx * dx + dz * dz;
    if seg_len_sq < 1e-4 {
        return 0;
    }

    let (h_start, h_end) = if let Some(hm) = map.heightmap.as_ref() {
        let hs = hm.sample_vertex(
            start_tile[0].clamp(0, map.width as i32) as u32,
            start_tile[1].clamp(0, map.height as i32) as u32,
        );
        let he = hm.sample_vertex(
            end_tile[0].clamp(0, map.width as i32) as u32,
            end_tile[1].clamp(0, map.height as i32) as u32,
        );
        if (he - hs).abs() < 0.15 {
            (hs, hs + 1.2)
        } else {
            (hs, he)
        }
    } else {
        (0.0, 1.2)
    };

    let core_r = half_width_tiles.max(0.75) * ts;
    let blend_r = (half_width_tiles.max(0.75) + 0.75) * ts;
    let vw = map.width + 1;
    let vh = map.height + 1;

    if let (Some(hm), Some(sm)) = (map.heightmap.as_mut(), map.splatmap.as_mut()) {
        for vz in 0..vh {
            for vx in 0..vw {
                let wx = vx as f32 * ts;
                let wz = vz as f32 * ts;
                let proj_t = (((wx - sx) * dx + (wz - sz) * dz) / seg_len_sq).clamp(0.0, 1.0);
                let px = sx + dx * proj_t;
                let pz = sz + dz * proj_t;
                let dist = ((wx - px) * (wx - px) + (wz - pz) * (wz - pz)).sqrt();
                if dist > blend_r {
                    continue;
                }
                let falloff = if dist <= core_r {
                    1.0
                } else {
                    (1.0 - (dist - core_r) / (blend_r - core_r).max(0.001)).clamp(0.0, 1.0)
                };
                // Smoothstep along ramp longitudinal axis for natural transition
                let smooth_u = proj_t * proj_t * (3.0 - 2.0 * proj_t);
                let target_h = h_start + (h_end - h_start) * smooth_u;
                let idx = (vz * vw + vx) as usize;
                if let Some(h) = hm.heights.get_mut(idx) {
                    *h = (*h + (target_h - *h) * falloff).clamp(-1.5, 6.0);
                }
                if let Some(rgba) = sm.weights_rgba.get_mut(idx) {
                    blend_splat_rgba_toward_layer(rgba, 1, falloff * 0.75);
                }
            }
        }
    }

    let mut ramp_count = 0usize;
    for tz in 0..(map.height as i32) {
        for tx in 0..(map.width as i32) {
            let cx = (tx as f32 + 0.5) * ts;
            let cz = (tz as f32 + 0.5) * ts;
            let raw_t = ((cx - sx) * dx + (cz - sz) * dz) / seg_len_sq;
            if !(0.0..=1.0).contains(&raw_t) {
                continue;
            }
            let px = sx + dx * raw_t;
            let pz = sz + dz * raw_t;
            let dist = ((cx - px) * (cx - px) + (cz - pz) * (cz - pz)).sqrt();
            if dist <= core_r {
                let tile = [tx, tz];
                world.grid.blocked_tiles.remove(&tile);
                world.grid.water_tiles.remove(&tile);
                world.grid.ramp_tiles.insert(tile);
                ramp_count += 1;
            }
        }
    }

    sync_grid_from_map_heightmap(map, world);
    ramp_count
}
