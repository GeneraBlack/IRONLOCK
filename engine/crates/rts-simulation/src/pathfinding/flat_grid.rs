use serde::{Deserialize, Deserializer, Serialize, Serializer};

const AUTO_GRID_MAX_DIM: i32 = 1024;

/// Cache-local flat `Vec<u8>` grid set with deterministic lexicographical `[i32; 2]` iteration.
/// Replaces `BTreeSet<[i32; 2]>` inside `CollisionGrid` (`Task 234`).
#[derive(Debug, Clone, Default)]
pub struct FlatTileSet {
    width: i32,
    height: i32,
    cells: Vec<u8>,
    coords: Vec<[i32; 2]>,
}

impl PartialEq for FlatTileSet {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.coords == other.coords
    }
}

impl Eq for FlatTileSet {}

impl FlatTileSet {
    pub fn with_dimensions(width: i32, height: i32) -> Self {
        let w = width.max(0);
        let h = height.max(0);
        let total = (w as usize).saturating_mul(h as usize);
        Self {
            width: w,
            height: h,
            cells: vec![0; total],
            coords: Vec::new(),
        }
    }

    pub fn resize_grid(&mut self, width: i32, height: i32) {
        let w = width.max(0);
        let h = height.max(0);
        if self.width == w && self.height == h && self.cells.len() == (w as usize) * (h as usize) {
            return;
        }
        self.width = w;
        self.height = h;
        let total = (w as usize).saturating_mul(h as usize);
        self.cells.clear();
        self.cells.resize(total, 0);
        let w_u = w as usize;
        for &[tx, tz] in &self.coords {
            if tx >= 0 && tx < w && tz >= 0 && tz < h {
                self.cells[(tz as usize) * w_u + (tx as usize)] = 1;
            }
        }
    }

    #[inline]
    fn cell_index(&self, tile: [i32; 2]) -> Option<usize> {
        if tile[0] >= 0
            && tile[0] < self.width
            && tile[1] >= 0
            && tile[1] < self.height
            && self.cells.len() == (self.width as usize).saturating_mul(self.height as usize)
        {
            Some((tile[1] as usize) * (self.width as usize) + (tile[0] as usize))
        } else {
            None
        }
    }

    fn ensure_covers(&mut self, tile: [i32; 2]) {
        if tile[0] >= 0
            && tile[1] >= 0
            && tile[0] < AUTO_GRID_MAX_DIM
            && tile[1] < AUTO_GRID_MAX_DIM
            && (tile[0] >= self.width || tile[1] >= self.height)
        {
            let nw = self.width.max(tile[0] + 1);
            let nh = self.height.max(tile[1] + 1);
            self.resize_grid(nw, nh);
        }
    }

    #[inline]
    pub fn contains(&self, tile: &[i32; 2]) -> bool {
        if self.coords.is_empty() {
            return false;
        }
        if let Some(idx) = self.cell_index(*tile) {
            self.cells[idx] != 0
        } else {
            self.coords.binary_search(tile).is_ok()
        }
    }

    pub fn insert(&mut self, tile: [i32; 2]) -> bool {
        self.ensure_covers(tile);
        if let Some(idx) = self.cell_index(tile) {
            if self.cells[idx] != 0 {
                return false;
            }
            self.cells[idx] = 1;
        }
        match self.coords.binary_search(&tile) {
            Ok(_) => false,
            Err(pos) => {
                self.coords.insert(pos, tile);
                true
            }
        }
    }

    pub fn remove(&mut self, tile: &[i32; 2]) -> bool {
        if let Some(idx) = self.cell_index(*tile) {
            if self.cells[idx] == 0 {
                return false;
            }
            self.cells[idx] = 0;
        }
        match self.coords.binary_search(tile) {
            Ok(pos) => {
                self.coords.remove(pos);
                true
            }
            Err(_) => false,
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.cells.fill(0);
        self.coords.clear();
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.coords.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.coords.is_empty()
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, [i32; 2]> {
        self.coords.iter()
    }

    #[inline]
    pub fn as_slice(&self) -> &[[i32; 2]] {
        &self.coords
    }

    pub fn retain<F: FnMut(&[i32; 2]) -> bool>(&mut self, mut f: F) {
        let w = self.width;
        let h = self.height;
        let w_u = w.max(0) as usize;
        let has_grid = self.cells.len() == w_u.saturating_mul(h.max(0) as usize);
        let cells = &mut self.cells;
        self.coords.retain(|t| {
            let keep = f(t);
            if !keep && has_grid && t[0] >= 0 && t[0] < w && t[1] >= 0 && t[1] < h {
                cells[(t[1] as usize) * w_u + (t[0] as usize)] = 0;
            }
            keep
        });
    }
}

impl<'a> IntoIterator for &'a FlatTileSet {
    type Item = &'a [i32; 2];
    type IntoIter = std::slice::Iter<'a, [i32; 2]>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.coords.iter()
    }
}

impl IntoIterator for FlatTileSet {
    type Item = [i32; 2];
    type IntoIter = std::vec::IntoIter<[i32; 2]>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.coords.into_iter()
    }
}

impl FromIterator<[i32; 2]> for FlatTileSet {
    fn from_iter<T: IntoIterator<Item = [i32; 2]>>(iter: T) -> Self {
        let mut coords: Vec<[i32; 2]> = iter.into_iter().collect();
        coords.sort_unstable();
        coords.dedup();
        let mut max_x = -1_i32;
        let mut max_z = -1_i32;
        for &[tx, tz] in &coords {
            if (0..AUTO_GRID_MAX_DIM).contains(&tx) && tx > max_x {
                max_x = tx;
            }
            if (0..AUTO_GRID_MAX_DIM).contains(&tz) && tz > max_z {
                max_z = tz;
            }
        }
        let mut set = Self {
            width: 0,
            height: 0,
            cells: Vec::new(),
            coords,
        };
        if max_x >= 0 && max_z >= 0 {
            set.resize_grid(max_x + 1, max_z + 1);
        }
        set
    }
}

impl Extend<[i32; 2]> for FlatTileSet {
    fn extend<T: IntoIterator<Item = [i32; 2]>>(&mut self, iter: T) {
        for tile in iter {
            self.insert(tile);
        }
    }
}

impl Serialize for FlatTileSet {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.coords.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for FlatTileSet {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let vec = Vec::<[i32; 2]>::deserialize(deserializer)?;
        Ok(vec.into_iter().collect())
    }
}

const EMPTY_SLOT: u32 = u32::MAX;

/// Cache-local flat `Vec<u32>`-indexed tile map storing sorted `([i32; 2], V)` entries.
/// Replaces `BTreeMap<[i32; 2], V>` inside `CollisionGrid` (`Task 234`).
#[derive(Debug, Clone)]
pub struct FlatTileMap<V> {
    width: i32,
    height: i32,
    slot_index: Vec<u32>,
    entries: Vec<([i32; 2], V)>,
}

impl<V> Default for FlatTileMap<V> {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            slot_index: Vec::new(),
            entries: Vec::new(),
        }
    }
}

impl<V: PartialEq> PartialEq for FlatTileMap<V> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl<V: Eq> Eq for FlatTileMap<V> {}

pub enum FlatTileEntry<'a, V> {
    Occupied(&'a mut V),
    Vacant {
        map: &'a mut FlatTileMap<V>,
        tile: [i32; 2],
    },
}

impl<'a, V> FlatTileEntry<'a, V> {
    pub fn or_insert(self, default: V) -> &'a mut V {
        match self {
            Self::Occupied(v) => v,
            Self::Vacant { map, tile } => map.insert_and_get_mut(tile, default),
        }
    }
}

impl<V> FlatTileMap<V> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_dimensions(width: i32, height: i32) -> Self {
        let w = width.max(0);
        let h = height.max(0);
        let total = (w as usize).saturating_mul(h as usize);
        Self {
            width: w,
            height: h,
            slot_index: vec![EMPTY_SLOT; total],
            entries: Vec::new(),
        }
    }

    pub fn resize_grid(&mut self, width: i32, height: i32) {
        let w = width.max(0);
        let h = height.max(0);
        if self.width == w
            && self.height == h
            && self.slot_index.len() == (w as usize) * (h as usize)
        {
            return;
        }
        self.width = w;
        self.height = h;
        let total = (w as usize).saturating_mul(h as usize);
        self.slot_index.clear();
        self.slot_index.resize(total, EMPTY_SLOT);
        self.rebuild_slot_indices_from(0);
    }

    fn rebuild_slot_indices_from(&mut self, start_idx: usize) {
        let w = self.width;
        let h = self.height;
        let w_u = w.max(0) as usize;
        if self.slot_index.len() != w_u.saturating_mul(h.max(0) as usize) {
            return;
        }
        for (idx, (tile, _)) in self.entries.iter().enumerate().skip(start_idx) {
            if tile[0] >= 0 && tile[0] < w && tile[1] >= 0 && tile[1] < h {
                self.slot_index[(tile[1] as usize) * w_u + (tile[0] as usize)] = idx as u32;
            }
        }
    }

    #[inline]
    fn cell_index(&self, tile: [i32; 2]) -> Option<usize> {
        if tile[0] >= 0
            && tile[0] < self.width
            && tile[1] >= 0
            && tile[1] < self.height
            && self.slot_index.len() == (self.width as usize).saturating_mul(self.height as usize)
        {
            Some((tile[1] as usize) * (self.width as usize) + (tile[0] as usize))
        } else {
            None
        }
    }

    fn ensure_covers(&mut self, tile: [i32; 2]) {
        if tile[0] >= 0
            && tile[1] >= 0
            && tile[0] < AUTO_GRID_MAX_DIM
            && tile[1] < AUTO_GRID_MAX_DIM
            && (tile[0] >= self.width || tile[1] >= self.height)
        {
            let nw = self.width.max(tile[0] + 1);
            let nh = self.height.max(tile[1] + 1);
            self.resize_grid(nw, nh);
        }
    }

    #[inline]
    pub fn contains_key(&self, tile: &[i32; 2]) -> bool {
        if self.entries.is_empty() {
            return false;
        }
        if let Some(cidx) = self.cell_index(*tile) {
            self.slot_index[cidx] != EMPTY_SLOT
        } else {
            self.entries.binary_search_by_key(tile, |(k, _)| *k).is_ok()
        }
    }

    #[inline]
    pub fn get(&self, tile: &[i32; 2]) -> Option<&V> {
        if self.entries.is_empty() {
            return None;
        }
        if let Some(cidx) = self.cell_index(*tile) {
            let slot = self.slot_index[cidx];
            if slot != EMPTY_SLOT {
                return self.entries.get(slot as usize).map(|(_, v)| v);
            }
            return None;
        }
        let pos = self.entries.binary_search_by_key(tile, |(k, _)| *k).ok()?;
        Some(&self.entries[pos].1)
    }

    #[inline]
    pub fn get_mut(&mut self, tile: &[i32; 2]) -> Option<&mut V> {
        if let Some(cidx) = self.cell_index(*tile) {
            let slot = self.slot_index[cidx];
            if slot != EMPTY_SLOT {
                return self.entries.get_mut(slot as usize).map(|(_, v)| v);
            }
            return None;
        }
        let pos = self.entries.binary_search_by_key(tile, |(k, _)| *k).ok()?;
        Some(&mut self.entries[pos].1)
    }

    pub fn insert(&mut self, tile: [i32; 2], value: V) -> Option<V> {
        self.ensure_covers(tile);
        if let Some(cidx) = self.cell_index(tile) {
            let slot = self.slot_index[cidx];
            if slot != EMPTY_SLOT {
                let prev = std::mem::replace(&mut self.entries[slot as usize].1, value);
                return Some(prev);
            }
        }
        match self.entries.binary_search_by_key(&tile, |(k, _)| *k) {
            Ok(pos) => Some(std::mem::replace(&mut self.entries[pos].1, value)),
            Err(pos) => {
                self.entries.insert(pos, (tile, value));
                self.rebuild_slot_indices_from(pos);
                None
            }
        }
    }

    fn insert_and_get_mut(&mut self, tile: [i32; 2], value: V) -> &mut V {
        self.ensure_covers(tile);
        let pos = match self.entries.binary_search_by_key(&tile, |(k, _)| *k) {
            Ok(p) => {
                self.entries[p].1 = value;
                p
            }
            Err(p) => {
                self.entries.insert(p, (tile, value));
                self.rebuild_slot_indices_from(p);
                p
            }
        };
        &mut self.entries[pos].1
    }

    pub fn entry(&mut self, tile: [i32; 2]) -> FlatTileEntry<'_, V> {
        if let Some(cidx) = self.cell_index(tile) {
            let slot = self.slot_index[cidx];
            if slot != EMPTY_SLOT {
                return FlatTileEntry::Occupied(&mut self.entries[slot as usize].1);
            }
            return FlatTileEntry::Vacant { map: self, tile };
        }
        if let Ok(pos) = self.entries.binary_search_by_key(&tile, |(k, _)| *k) {
            FlatTileEntry::Occupied(&mut self.entries[pos].1)
        } else {
            FlatTileEntry::Vacant { map: self, tile }
        }
    }

    pub fn remove(&mut self, tile: &[i32; 2]) -> Option<V> {
        if let Some(cidx) = self.cell_index(*tile) {
            let slot = self.slot_index[cidx];
            if slot == EMPTY_SLOT {
                return None;
            }
            self.slot_index[cidx] = EMPTY_SLOT;
            let pos = slot as usize;
            let (_, val) = self.entries.remove(pos);
            self.rebuild_slot_indices_from(pos);
            return Some(val);
        }
        let pos = self.entries.binary_search_by_key(tile, |(k, _)| *k).ok()?;
        let (_, val) = self.entries.remove(pos);
        self.rebuild_slot_indices_from(pos);
        Some(val)
    }

    #[inline]
    pub fn clear(&mut self) {
        self.slot_index.fill(EMPTY_SLOT);
        self.entries.clear();
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[inline]
    pub fn iter(&self) -> FlatTileMapIter<'_, V> {
        FlatTileMapIter {
            inner: self.entries.iter(),
        }
    }

    #[inline]
    pub fn keys(&self) -> impl Iterator<Item = &[i32; 2]> {
        self.entries.iter().map(|(k, _)| k)
    }

    #[inline]
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.entries.iter().map(|(_, v)| v)
    }
}

pub struct FlatTileMapIter<'a, V> {
    inner: std::slice::Iter<'a, ([i32; 2], V)>,
}

impl<'a, V> Iterator for FlatTileMapIter<'a, V> {
    type Item = (&'a [i32; 2], &'a V);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(k, v)| (k, v))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a, V> ExactSizeIterator for FlatTileMapIter<'a, V> {}

impl<'a, V> IntoIterator for &'a FlatTileMap<V> {
    type Item = (&'a [i32; 2], &'a V);
    type IntoIter = FlatTileMapIter<'a, V>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<V> IntoIterator for FlatTileMap<V> {
    type Item = ([i32; 2], V);
    type IntoIter = std::vec::IntoIter<([i32; 2], V)>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl<V> FromIterator<([i32; 2], V)> for FlatTileMap<V> {
    fn from_iter<T: IntoIterator<Item = ([i32; 2], V)>>(iter: T) -> Self {
        let mut entries: Vec<([i32; 2], V)> = iter.into_iter().collect();
        entries.sort_by_key(|(k, _)| *k);
        // Keep last value on duplicate keys just like BTreeMap::from_iter
        let mut deduped: Vec<([i32; 2], V)> = Vec::with_capacity(entries.len());
        for (k, v) in entries {
            if let Some((last_k, last_v)) = deduped.last_mut() {
                if *last_k == k {
                    *last_v = v;
                    continue;
                }
            }
            deduped.push((k, v));
        }
        let mut max_x = -1_i32;
        let mut max_z = -1_i32;
        for &(tile, _) in &deduped {
            if tile[0] >= 0 && tile[0] < AUTO_GRID_MAX_DIM && tile[0] > max_x {
                max_x = tile[0];
            }
            if tile[1] >= 0 && tile[1] < AUTO_GRID_MAX_DIM && tile[1] > max_z {
                max_z = tile[1];
            }
        }
        let mut map = Self {
            width: 0,
            height: 0,
            slot_index: Vec::new(),
            entries: deduped,
        };
        if max_x >= 0 && max_z >= 0 {
            map.resize_grid(max_x + 1, max_z + 1);
        }
        map
    }
}

impl<V: Serialize> Serialize for FlatTileMap<V> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.entries.serialize(serializer)
    }
}

impl<'de, V: Deserialize<'de>> Deserialize<'de> for FlatTileMap<V> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let vec = Vec::<([i32; 2], V)>::deserialize(deserializer)?;
        Ok(vec.into_iter().collect())
    }
}
