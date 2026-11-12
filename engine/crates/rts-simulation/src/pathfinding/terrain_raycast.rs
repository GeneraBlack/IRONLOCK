use crate::det_math::{quantize_f32_to_sim, SimScalar};
use crate::pathfinding::{CollisionGrid, GridHeightCache};
use rts_sdk::schema::{MapDef, MovementDomain};

impl CollisionGrid {
    /// Synchronizes `(width + 1) * (height + 1)` continuous `vertex_heights` from `MapDef`
    /// (live Studio terrain sculpting or map load) and refreshes step-clearance topology.
    pub fn sync_heightmap_from_map_def(&mut self, map: &MapDef) {
        let vw = (self.width.max(0) as usize) + 1;
        let vh = (self.height.max(0) as usize) + 1;
        let expected = vw.saturating_mul(vh);
        if self.width <= 0 || self.height <= 0 || expected == 0 {
            self.vertex_heights.clear();
            self.rebuild_height_cache();
            return;
        }
        let (hm, _) = map.synthesize_height_and_splat();
        if hm.width as usize == vw && hm.height as usize == vh && hm.heights.len() == expected {
            self.vertex_heights = hm
                .heights
                .into_iter()
                .map(|v| quantize_f32_to_sim(v).to_f32())
                .collect();
        } else {
            let mut resampled = Vec::with_capacity(expected);
            let denom_x = (vw.saturating_sub(1)).max(1) as f32;
            let denom_z = (vh.saturating_sub(1)).max(1) as f32;
            for vz in 0..vh {
                for vx in 0..vw {
                    let h = hm.sample_bilinear(vx as f32 / denom_x, vz as f32 / denom_z);
                    resampled.push(quantize_f32_to_sim(h).to_f32());
                }
            }
            self.vertex_heights = resampled;
        }
        self.rebuild_height_cache();
        self.rebuild_clearance_map();
    }

    /// Rebuilds the transient `O(1)` tile-center height lookup table and min/max relief bounds
    /// using exact `SimScalar` bilinear interpolation (`sample_raw_terrain_height`).
    pub fn rebuild_height_cache(&mut self) {
        if !self.has_valid_vertex_heights() {
            self.height_cache.tile_heights.clear();
            self.height_cache.min_vertex_height = 0.0;
            self.height_cache.max_vertex_height = 0.0;
            self.height_cache.synced_vertex_len = 0;
            self.height_cache.synced_width = 0;
            self.height_cache.synced_height = 0;
            return;
        }
        let mut min_h = f32::INFINITY;
        let mut max_h = f32::NEG_INFINITY;
        for &vh in &self.vertex_heights {
            if vh < min_h {
                min_h = vh;
            }
            if vh > max_h {
                max_h = vh;
            }
        }
        if !min_h.is_finite() {
            min_h = 0.0;
            max_h = 0.0;
        }
        let w = self.width as usize;
        let h = self.height as usize;
        let total = w.saturating_mul(h);
        let mut tile_heights = Vec::with_capacity(total);
        let ts = self.tile_size;
        for z in 0..self.height {
            let wz = (z as f32 + 0.5) * ts;
            for x in 0..self.width {
                let wx = (x as f32 + 0.5) * ts;
                tile_heights.push(self.sample_raw_terrain_height(wx, wz));
            }
        }
        self.height_cache = GridHeightCache {
            tile_heights,
            min_vertex_height: min_h,
            max_vertex_height: max_h,
            synced_vertex_len: self.vertex_heights.len(),
            synced_width: self.width,
            synced_height: self.height,
        };
    }

    #[inline]
    pub(crate) fn has_synced_height_cache(&self) -> bool {
        self.has_valid_vertex_heights()
            && self.height_cache.synced_vertex_len == self.vertex_heights.len()
            && self.height_cache.synced_width == self.width
            && self.height_cache.synced_height == self.height
            && self.height_cache.tile_heights.len()
                == (self.width as usize).saturating_mul(self.height as usize)
    }

    /// Returns `true` when no discrete cliff or sculpted terrain vertex on the map can possibly
    /// occlude a horizontal sightline originating at `from_elev`.
    #[inline]
    pub(crate) fn is_all_los_clear_from(&self, from_elev: f32) -> bool {
        if !self.elevation_tiles.is_empty() {
            return false;
        }
        if !self.has_valid_vertex_heights() {
            return self.ramp_tiles.is_empty() || from_elev + 0.85 >= 0.675;
        }
        self.has_synced_height_cache() && self.height_cache.max_vertex_height <= from_elev + 0.85
    }

    #[inline]
    pub fn has_valid_vertex_heights(&self) -> bool {
        if self.width <= 0 || self.height <= 0 {
            return false;
        }
        let expected = ((self.width as usize) + 1).saturating_mul((self.height as usize) + 1);
        self.vertex_heights.len() == expected
    }

    /// Samples a corner vertex `(vx, vz)` in `[0..=width, 0..=height]` from `vertex_heights`
    /// (falling back to adjacent discrete tile elevations when `vertex_heights` is unpopulated).
    #[inline]
    pub fn sample_vertex_height(&self, vx: i32, vz: i32) -> f32 {
        if self.has_valid_vertex_heights() {
            let cx = vx.clamp(0, self.width) as usize;
            let cz = vz.clamp(0, self.height) as usize;
            let vw = (self.width as usize) + 1;
            return self.vertex_heights[cz * vw + cx];
        }
        let mut sum = 0.0_f32;
        let mut cnt = 0_u32;
        for &(ox, oz) in &[(-1_i32, -1_i32), (0, -1), (-1, 0), (0, 0)] {
            let tx = vx + ox;
            let tz = vz + oz;
            if self.in_bounds([tx, tz]) {
                let t = [tx, tz];
                let h = if self.ramp_tiles.contains(&t) {
                    0.675
                } else if self.elevation_tiles.contains(&t) {
                    1.35
                } else if self.water_tiles.contains(&t) && !self.bridge_tiles.contains(&t) {
                    Self::WATER_SURFACE_Y
                } else {
                    0.0
                };
                sum += h;
                cnt += 1;
            }
        }
        if cnt > 0 {
            sum / (cnt as f32)
        } else {
            0.0
        }
    }

    /// Bilinearly interpolates continuous terrain height at `(world_x, world_z)` using `SimScalar`,
    /// returning `+0.55` (`BRIDGE_DECK_Y`) on intact `bridge_tiles` for ground surface queries.
    pub fn sample_world_height(&self, world_x: f32, world_z: f32) -> f32 {
        let tile = self.world_to_tile([world_x, 0.0, world_z]);
        if self.bridge_tiles.contains(&tile) {
            return Self::BRIDGE_DECK_Y;
        }
        self.sample_raw_terrain_height(world_x, world_z)
    }

    /// Bilinearly interpolates the underlying terrain/riverbed heightmap ignoring bridge decks.
    pub fn sample_raw_terrain_height(&self, world_x: f32, world_z: f32) -> f32 {
        if self.has_valid_vertex_heights() {
            let ts = SimScalar::from_f32(self.tile_size.max(0.01));
            let max_gx = SimScalar::from_num(self.width.max(0));
            let max_gz = SimScalar::from_num(self.height.max(0));
            let gx = (quantize_f32_to_sim(world_x) / ts).clamp(SimScalar::ZERO, max_gx);
            let gz = (quantize_f32_to_sim(world_z) / ts).clamp(SimScalar::ZERO, max_gz);

            let x0 = gx.floor_to_i32().clamp(0, (self.width - 1).max(0));
            let z0 = gz.floor_to_i32().clamp(0, (self.height - 1).max(0));
            let x1 = (x0 + 1).min(self.width);
            let z1 = (z0 + 1).min(self.height);

            let tx = (gx - SimScalar::from_num(x0)).clamp(SimScalar::ZERO, SimScalar::ONE);
            let tz = (gz - SimScalar::from_num(z0)).clamp(SimScalar::ZERO, SimScalar::ONE);

            let vw = (self.width as usize) + 1;
            let h00 = quantize_f32_to_sim(self.vertex_heights[(z0 as usize) * vw + (x0 as usize)]);
            let h10 = quantize_f32_to_sim(self.vertex_heights[(z0 as usize) * vw + (x1 as usize)]);
            let h01 = quantize_f32_to_sim(self.vertex_heights[(z1 as usize) * vw + (x0 as usize)]);
            let h11 = quantize_f32_to_sim(self.vertex_heights[(z1 as usize) * vw + (x1 as usize)]);

            let hx0 = h00 + (h10 - h00) * tx;
            let hx1 = h01 + (h11 - h01) * tx;
            return (hx0 + (hx1 - hx0) * tz).to_f32();
        }
        let tile = self.world_to_tile([world_x, 0.0, world_z]);
        if self.ramp_tiles.contains(&tile) {
            0.675
        } else if self.elevation_tiles.contains(&tile) {
            1.35
        } else if self.water_tiles.contains(&tile) {
            Self::WATER_SURFACE_Y
        } else {
            0.0
        }
    }

    /// Domain-aware surface height separating `Naval` ships on water (`y = -0.35`, including under bridges!)
    /// from `Ground` / `Amphibious` units on bridge decks (`y = +0.55`, `Δy = 0.90m`).
    pub fn sample_domain_surface_height(
        &self,
        world_x: f32,
        world_z: f32,
        domain: MovementDomain,
    ) -> f32 {
        let tile = self.world_to_tile([world_x, 0.0, world_z]);
        match domain {
            MovementDomain::Naval => {
                if self.water_tiles.contains(&tile) || self.water_tiles.is_empty() {
                    Self::WATER_SURFACE_Y
                } else {
                    self.sample_raw_terrain_height(world_x, world_z)
                }
            }
            MovementDomain::Amphibious => {
                if self.bridge_tiles.contains(&tile) {
                    Self::BRIDGE_DECK_Y
                } else if self.water_tiles.contains(&tile) {
                    Self::WATER_SURFACE_Y
                } else {
                    self.sample_world_height(world_x, world_z)
                }
            }
            _ => {
                if self.bridge_tiles.contains(&tile) {
                    Self::BRIDGE_DECK_Y
                } else {
                    self.sample_world_height(world_x, world_z)
                }
            }
        }
    }

    #[inline]
    pub fn tile_elevation(&self, tile: [i32; 2]) -> f32 {
        if !self.bridge_tiles.is_empty() && self.bridge_tiles.contains(&tile) {
            return Self::BRIDGE_DECK_Y;
        }
        if self.has_valid_vertex_heights() {
            if self.has_synced_height_cache() && self.in_bounds(tile) {
                return self.height_cache.tile_heights
                    [(tile[1] as usize) * (self.width as usize) + (tile[0] as usize)];
            }
            let wx = (tile[0] as f32 + 0.5) * self.tile_size;
            let wz = (tile[1] as f32 + 0.5) * self.tile_size;
            return self.sample_raw_terrain_height(wx, wz);
        }
        if !self.ramp_tiles.is_empty() && self.ramp_tiles.contains(&tile) {
            0.675
        } else if !self.elevation_tiles.is_empty() && self.elevation_tiles.contains(&tile) {
            1.35
        } else if !self.water_tiles.is_empty() && self.water_tiles.contains(&tile) {
            Self::WATER_SURFACE_Y
        } else {
            0.0
        }
    }

    /// Exact 3D ray vs. continuous heightfield + bridge-deck intersection using 2D grid DDA
    /// (Amanatides & Woo) and Möller–Trumbore triangle intersection per visited cell.
    pub fn raycast_heightfield(
        &self,
        origin: [f32; 3],
        dir: [f32; 3],
        max_dist: f32,
    ) -> Option<[f32; 3]> {
        if self.width <= 0 || self.height <= 0 || self.tile_size <= 0.0 || max_dist <= 0.0 {
            return None;
        }
        let len_sq = dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2];
        if !len_sq.is_finite() || len_sq <= 1e-12 {
            return None;
        }
        let inv_len = 1.0 / len_sq.sqrt();
        let rd = [dir[0] * inv_len, dir[1] * inv_len, dir[2] * inv_len];

        let ts = self.tile_size;
        let max_x = (self.width as f32) * ts;
        let max_z = (self.height as f32) * ts;

        let mut t_min = 0.0_f32;
        let mut t_max = max_dist;

        for &(orig_c, dir_c, b_min, b_max) in &[
            (origin[0], rd[0], 0.0_f32, max_x),
            (origin[2], rd[2], 0.0_f32, max_z),
        ] {
            if dir_c.abs() < 1e-8 {
                if orig_c < b_min || orig_c > b_max {
                    return None;
                }
            } else {
                let t0 = (b_min - orig_c) / dir_c;
                let t1 = (b_max - orig_c) / dir_c;
                let (t_near, t_far) = if t0 <= t1 { (t0, t1) } else { (t1, t0) };
                t_min = t_min.max(t_near);
                t_max = t_max.min(t_far);
                if t_min > t_max {
                    return None;
                }
            }
        }

        let t_entry = (t_min + 1e-5).min(t_max);
        let entry_x = (origin[0] + rd[0] * t_entry).clamp(0.0, (max_x - 1e-5).max(0.0));
        let entry_z = (origin[2] + rd[2] * t_entry).clamp(0.0, (max_z - 1e-5).max(0.0));

        let mut cx = ((entry_x / ts).floor() as i32).clamp(0, self.width - 1);
        let mut cz = ((entry_z / ts).floor() as i32).clamp(0, self.height - 1);

        let step_x = if rd[0] > 1e-8 {
            1
        } else if rd[0] < -1e-8 {
            -1
        } else {
            0
        };
        let step_z = if rd[2] > 1e-8 {
            1
        } else if rd[2] < -1e-8 {
            -1
        } else {
            0
        };

        let t_delta_x = if step_x != 0 {
            (ts / rd[0]).abs()
        } else {
            f32::INFINITY
        };
        let t_delta_z = if step_z != 0 {
            (ts / rd[2]).abs()
        } else {
            f32::INFINITY
        };

        let next_bx = if step_x > 0 {
            ((cx + 1) as f32) * ts
        } else {
            (cx as f32) * ts
        };
        let next_bz = if step_z > 0 {
            ((cz + 1) as f32) * ts
        } else {
            (cz as f32) * ts
        };

        let mut t_max_x = if step_x != 0 {
            ((next_bx - origin[0]) / rd[0]).max(t_min)
        } else {
            f32::INFINITY
        };
        let mut t_max_z = if step_z != 0 {
            ((next_bz - origin[2]) / rd[2]).max(t_min)
        } else {
            f32::INFINITY
        };

        let mut best_t: Option<f32> = None;
        let mut t_curr_enter = t_min;
        let max_iters = ((self.width + self.height) as usize) * 3 + 16;

        for _ in 0..max_iters {
            if cx < 0 || cx >= self.width || cz < 0 || cz >= self.height || t_curr_enter > t_max {
                break;
            }
            if let Some(bt) = best_t {
                if t_curr_enter > bt + 1e-4 {
                    break;
                }
            }

            let x0 = (cx as f32) * ts;
            let x1 = ((cx + 1) as f32) * ts;
            let z0 = (cz as f32) * ts;
            let z1 = ((cz + 1) as f32) * ts;

            let h00 = self.sample_vertex_height(cx, cz);
            let h10 = self.sample_vertex_height(cx + 1, cz);
            let h01 = self.sample_vertex_height(cx, cz + 1);
            let h11 = self.sample_vertex_height(cx + 1, cz + 1);

            let v00 = [x0, h00, z0];
            let v10 = [x1, h10, z0];
            let v01 = [x0, h01, z1];
            let v11 = [x1, h11, z1];

            for (a, b, c) in [(v00, v01, v10), (v10, v01, v11)] {
                if let Some(t_hit) = ray_intersect_triangle_mt(origin, rd, a, b, c, max_dist) {
                    if best_t.map(|bt| t_hit < bt).unwrap_or(true) {
                        best_t = Some(t_hit);
                    }
                }
            }

            if self.bridge_tiles.contains(&[cx, cz]) {
                let by = Self::BRIDGE_DECK_Y;
                let b00 = [x0, by, z0];
                let b10 = [x1, by, z0];
                let b01 = [x0, by, z1];
                let b11 = [x1, by, z1];
                for (a, b, c) in [(b00, b01, b10), (b10, b01, b11)] {
                    if let Some(t_hit) = ray_intersect_triangle_mt(origin, rd, a, b, c, max_dist) {
                        if best_t.map(|bt| t_hit < bt).unwrap_or(true) {
                            best_t = Some(t_hit);
                        }
                    }
                }
            }

            if step_x == 0 && step_z == 0 {
                break;
            }
            if t_max_x < t_max_z {
                t_curr_enter = t_max_x;
                t_max_x += t_delta_x;
                cx += step_x;
            } else {
                t_curr_enter = t_max_z;
                t_max_z += t_delta_z;
                cz += step_z;
            }
        }

        best_t.map(|t| {
            [
                origin[0] + rd[0] * t,
                origin[1] + rd[1] * t,
                origin[2] + rd[2] * t,
            ]
        })
    }

    /// Multi-level High-Ground Line-of-Sight occlusion across both discrete `elevation_tiles`
    /// and continuous sculpted `vertex_heights`: observers cannot see up onto steeper plateaus
    /// (`to_elev > from_elev + 0.85`) unless standing on a ramp, nor see through intermediate
    /// taller ridges/cliffs (`mid_elev > max(from_elev, to_elev) + 0.85`).
    #[inline]
    pub fn has_line_of_sight(
        &self,
        from_tile: [i32; 2],
        from_elev: f32,
        to_tile: [i32; 2],
    ) -> bool {
        if from_tile == to_tile || self.is_all_los_clear_from(from_elev) {
            return true;
        }
        let has_ramps = !self.ramp_tiles.is_empty();
        let has_bridges = !self.bridge_tiles.is_empty();
        let has_elevs = !self.elevation_tiles.is_empty();
        let has_vh = self.has_valid_vertex_heights();
        let on_ramp = has_ramps && self.ramp_tiles.contains(&from_tile);
        let to_elev = self.tile_elevation(to_tile);
        if !(has_bridges && self.bridge_tiles.contains(&to_tile))
            && to_elev > from_elev + 0.85
            && !on_ramp
        {
            return false;
        }
        if !has_vh && !has_elevs {
            return true;
        }
        let ceiling = from_elev.max(to_elev) + 0.85;
        if !has_elevs
            && self.has_synced_height_cache()
            && self.height_cache.max_vertex_height <= ceiling
        {
            return true;
        }
        let mut x0 = from_tile[0];
        let mut y0 = from_tile[1];
        let x1 = to_tile[0];
        let y1 = to_tile[1];
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        for _ in 0..256 {
            if (x0 != from_tile[0] || y0 != from_tile[1]) && (x0 != x1 || y0 != y1) {
                let mid = [x0, y0];
                let mid_exempt = (has_ramps && self.ramp_tiles.contains(&mid))
                    || (has_bridges && self.bridge_tiles.contains(&mid));
                if !mid_exempt {
                    if has_elevs
                        && self.elevation_tiles.contains(&mid)
                        && from_elev < 0.6
                        && !on_ramp
                    {
                        return false;
                    }
                    if has_vh && self.tile_elevation(mid) > ceiling {
                        return false;
                    }
                }
            }
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
        true
    }
}

#[inline]
fn ray_intersect_triangle_mt(
    origin: [f32; 3],
    dir: [f32; 3],
    v0: [f32; 3],
    v1: [f32; 3],
    v2: [f32; 3],
    max_dist: f32,
) -> Option<f32> {
    let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
    let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
    let pvec = [
        dir[1] * e2[2] - dir[2] * e2[1],
        dir[2] * e2[0] - dir[0] * e2[2],
        dir[0] * e2[1] - dir[1] * e2[0],
    ];
    let det = e1[0] * pvec[0] + e1[1] * pvec[1] + e1[2] * pvec[2];
    if det.abs() < 1e-7 {
        return None;
    }
    let inv_det = 1.0 / det;
    let tvec = [origin[0] - v0[0], origin[1] - v0[1], origin[2] - v0[2]];
    let u = (tvec[0] * pvec[0] + tvec[1] * pvec[1] + tvec[2] * pvec[2]) * inv_det;
    let eps = 1e-4_f32;
    if u < -eps || u > 1.0 + eps {
        return None;
    }
    let qvec = [
        tvec[1] * e1[2] - tvec[2] * e1[1],
        tvec[2] * e1[0] - tvec[0] * e1[2],
        tvec[0] * e1[1] - tvec[1] * e1[0],
    ];
    let v = (dir[0] * qvec[0] + dir[1] * qvec[1] + dir[2] * qvec[2]) * inv_det;
    if v < -eps || u + v > 1.0 + eps {
        return None;
    }
    let t = (e2[0] * qvec[0] + e2[1] * qvec[1] + e2[2] * qvec[2]) * inv_det;
    if t >= 0.0 && t <= max_dist {
        Some(t)
    } else {
        None
    }
}
