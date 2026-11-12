use std::collections::BTreeMap;

pub type SpatialCellEntry = (u64, [f32; 3], u8);

/// Deterministic 2D spatial hash grid for fast $O(1)$ neighborhood & radius queries
/// across units and structures (used by combat target acquisition, splash damage, and soft collision separation).
#[derive(Debug, Clone, Default)]
pub struct SpatialHashGrid {
    pub cell_size: f32,
    pub cells: BTreeMap<[i32; 2], Vec<SpatialCellEntry>>,
}

impl SpatialHashGrid {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size: cell_size.max(1.0),
            cells: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, id: u64, pos: [f32; 3], player_id: u8) {
        let cell = self.pos_to_cell(pos);
        self.cells
            .entry(cell)
            .or_default()
            .push((id, pos, player_id));
    }

    pub fn pos_to_cell(&self, pos: [f32; 3]) -> [i32; 2] {
        [
            (pos[0] / self.cell_size).floor() as i32,
            (pos[2] / self.cell_size).floor() as i32,
        ]
    }

    pub fn query_radius(&self, center: [f32; 3], radius: f32) -> Vec<(u64, [f32; 3], u8)> {
        let c = self.pos_to_cell(center);
        let r_cells = (radius / self.cell_size).ceil() as i32;
        let r_sq = radius * radius;
        let mut out = Vec::new();
        for dz in -r_cells..=r_cells {
            for dx in -r_cells..=r_cells {
                if let Some(entries) = self.cells.get(&[c[0] + dx, c[1] + dz]) {
                    for &(id, pos, pid) in entries {
                        let ddx = pos[0] - center[0];
                        let ddz = pos[2] - center[2];
                        if ddx * ddx + ddz * ddz <= r_sq {
                            out.push((id, pos, pid));
                        }
                    }
                }
            }
        }
        out
    }
}

/// Entry stored inside the persistent flat `WorldSpatialIndex`.
#[derive(Debug, Clone, Copy)]
pub struct SpatialIndexEntry {
    pub id: u64,
    pub pos: [f32; 3],
    pub player_id: u8,
    pub is_building: bool,
    pub is_air: bool,
    pub next: i32,
}

/// Persistent, zero-allocation flat 2D spatial index (`cell_head` + `cell_tail` + `entries` linked list)
/// shared across Combat, ORCA, Projectiles, Crushing, Mines, Gates, and Healers.
#[derive(Debug, Clone)]
pub struct WorldSpatialIndex {
    pub cell_size: f32,
    pub origin_pad_cells: i32,
    pub grid_cols: i32,
    pub grid_rows: i32,
    pub cell_head: Vec<i32>,
    cell_tail: Vec<i32>,
    pub entries: Vec<SpatialIndexEntry>,
}

impl Default for WorldSpatialIndex {
    fn default() -> Self {
        Self {
            cell_size: 8.0,
            origin_pad_cells: 2,
            grid_cols: 0,
            grid_rows: 0,
            cell_head: Vec::new(),
            cell_tail: Vec::new(),
            entries: Vec::new(),
        }
    }
}

impl WorldSpatialIndex {
    pub fn reset_in_place(
        &mut self,
        world_width_m: f32,
        world_height_m: f32,
        cell_size: f32,
        entity_capacity: usize,
    ) {
        let cs = cell_size.max(2.0);
        let pad = 2_i32;
        let cols = ((world_width_m.max(cs) / cs).ceil() as i32 + pad * 2).max(4);
        let rows = ((world_height_m.max(cs) / cs).ceil() as i32 + pad * 2).max(4);
        let cell_count = (cols as usize).saturating_mul(rows as usize);

        self.cell_size = cs;
        self.origin_pad_cells = pad;
        self.grid_cols = cols;
        self.grid_rows = rows;

        if self.cell_head.len() != cell_count {
            self.cell_head.resize(cell_count, -1);
            self.cell_tail.resize(cell_count, -1);
        }
        self.cell_head.fill(-1);
        self.cell_tail.fill(-1);

        self.entries.clear();
        if self.entries.capacity() < entity_capacity {
            self.entries
                .reserve(entity_capacity - self.entries.capacity());
        }
    }

    #[inline]
    fn world_to_cell_coords(&self, x: f32, z: f32) -> [i32; 2] {
        let cx = ((x / self.cell_size).floor() as i32 + self.origin_pad_cells)
            .clamp(0, (self.grid_cols - 1).max(0));
        let cz = ((z / self.cell_size).floor() as i32 + self.origin_pad_cells)
            .clamp(0, (self.grid_rows - 1).max(0));
        [cx, cz]
    }

    #[inline]
    pub fn insert(
        &mut self,
        id: u64,
        pos: [f32; 3],
        player_id: u8,
        is_building: bool,
        is_air: bool,
    ) {
        if self.grid_cols <= 0 || self.grid_rows <= 0 {
            self.reset_in_place(256.0, 256.0, 8.0, 64);
        }
        let [cx, cz] = self.world_to_cell_coords(pos[0], pos[2]);
        let cell_idx = (cz as usize) * (self.grid_cols as usize) + (cx as usize);
        let entry_idx = self.entries.len() as i32;
        self.entries.push(SpatialIndexEntry {
            id,
            pos,
            player_id,
            is_building,
            is_air,
            next: -1,
        });
        let prev_tail = self.cell_tail[cell_idx];
        if prev_tail < 0 {
            self.cell_head[cell_idx] = entry_idx;
            self.cell_tail[cell_idx] = entry_idx;
        } else {
            self.entries[prev_tail as usize].next = entry_idx;
            self.cell_tail[cell_idx] = entry_idx;
        }
    }

    /// Rebuilds the index in-place from `(id, pos, player_id)` tuples with zero heap allocations once warmed.
    pub fn rebuild<I>(
        &mut self,
        grid_width: i32,
        grid_height: i32,
        tile_size: f32,
        cell_size: f32,
        iter: I,
    ) where
        I: IntoIterator<Item = (u64, [f32; 3], u8)>,
    {
        let it = iter.into_iter();
        let (lo, _) = it.size_hint();
        let ts = tile_size.max(1.0);
        self.reset_in_place(
            (grid_width.max(4) as f32) * ts,
            (grid_height.max(4) as f32) * ts,
            cell_size.max(2.0),
            lo.max(16),
        );
        for (id, pos, player_id) in it {
            self.insert(id, pos, player_id, false, false);
        }
    }

    /// Rebuilds the index in-place from `(id, player_id, pos)` tuples using current world bounds (or default `512m`).
    pub fn rebuild_from_iter<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = (u64, u8, [f32; 3])>,
    {
        let it = iter.into_iter();
        let (lo, _) = it.size_hint();
        let cs = if self.cell_size >= 2.0 {
            self.cell_size
        } else {
            8.0
        };
        let ww = if self.grid_cols > 4 {
            ((self.grid_cols - self.origin_pad_cells * 2).max(4) as f32) * cs
        } else {
            512.0
        };
        let wh = if self.grid_rows > 4 {
            ((self.grid_rows - self.origin_pad_cells * 2).max(4) as f32) * cs
        } else {
            512.0
        };
        self.reset_in_place(ww, wh, cs, lo.max(16));
        for (id, player_id, pos) in it {
            self.insert(id, pos, player_id, false, false);
        }
    }

    /// Zero-allocation callback traversal over every indexed entity within 2D `(x, z)` `radius` of `center`,
    /// passing `(id, pos, player_id)`. Entries within each cell are visited in exact ascending insertion (`id`) order.
    #[inline]
    pub fn for_each_in_radius<F>(&self, center: [f32; 3], radius: f32, mut f: F)
    where
        F: FnMut(u64, [f32; 3], u8),
    {
        self.for_each_entry_in_radius(center, radius, |entry| {
            f(entry.id, entry.pos, entry.player_id);
        });
    }

    /// Zero-allocation 2D `(x, z)` callback traversal passing `(id, player_id, pos)`.
    #[inline]
    pub fn for_each_in_radius_xz<F>(&self, center_xz: [f32; 2], radius: f32, mut f: F)
    where
        F: FnMut(u64, u8, [f32; 3]),
    {
        self.for_each_entry_in_radius([center_xz[0], 0.0, center_xz[1]], radius, |entry| {
            f(entry.id, entry.player_id, entry.pos);
        });
    }

    /// Zero-allocation callback traversal over every `SpatialIndexEntry` within 2D `(x, z)` `radius` of `center`.
    #[inline]
    pub fn for_each_entry_in_radius<F>(&self, center: [f32; 3], radius: f32, mut f: F)
    where
        F: FnMut(&SpatialIndexEntry),
    {
        if self.entries.is_empty() || self.grid_cols <= 0 || self.grid_rows <= 0 || radius < 0.0 {
            return;
        }
        let min_c = self.world_to_cell_coords(center[0] - radius, center[2] - radius);
        let max_c = self.world_to_cell_coords(center[0] + radius, center[2] + radius);
        let r_sq = radius * radius;
        let cols = self.grid_cols as usize;

        for cz in min_c[1]..=max_c[1] {
            let row_off = (cz as usize) * cols;
            for cx in min_c[0]..=max_c[0] {
                let mut cur = self.cell_head[row_off + (cx as usize)];
                while cur >= 0 {
                    let entry = &self.entries[cur as usize];
                    let dx = entry.pos[0] - center[0];
                    let dz = entry.pos[2] - center[2];
                    if dx * dx + dz * dz <= r_sq {
                        f(entry);
                    }
                    cur = entry.next;
                }
            }
        }
    }
}
