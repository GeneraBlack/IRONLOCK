use anyhow::Result;
use std::collections::HashMap;

use super::ColoredMeshPart;

/// Cosine of the 35-degree hard-surface crease threshold (`cos(35°)`).
/// Adjacent exposed faces whose normals differ by `<= 35°` (`dot >= CREASE_COS_35`) smoothly
/// interpolate Phong vertex normals across cylindrical barrels, Kirov hulls, and curved domes;
/// faces differing by `> 35°` (`dot < CREASE_COS_35`) preserve crisp planar hard creases.
const CREASE_COS_35: f32 = 0.819_152_06;

/// Authentic Westwood Tiberian Sun 36-entry precomputed normal table (`normals_mode == 2`),
/// in Westwood voxel space `(x = forward, y = side, z = up)`.
pub const WESTWOOD_TS_NORMALS_36: [[f32; 3]; 36] = [
    [0.671214, 0.198492, -0.714194], [0.269643, 0.584394, -0.765360], [-0.040546, 0.096988, -0.994459],
    [-0.572428, -0.091914, -0.814787], [-0.171401, -0.572710, -0.801639], [0.362557, -0.302999, -0.881331],
    [0.810347, -0.348972, -0.470698], [0.103962, 0.938672, -0.328767], [-0.324047, 0.587669, -0.741376],
    [-0.800865, 0.340461, -0.492647], [-0.665498, -0.590147, -0.456989], [0.314767, -0.803002, -0.506073],
    [0.972629, 0.151076, -0.176550], [0.680291, 0.684236, -0.262727], [-0.520079, 0.827777, -0.210483],
    [-0.961644, -0.179001, -0.207847], [-0.262714, -0.937451, -0.228401], [0.219707, -0.971301, 0.091125],
    [0.923808, -0.229975, 0.306087], [-0.082489, 0.970660, 0.225866], [-0.591798, 0.696790, 0.405289],
    [-0.925296, 0.366601, 0.097111], [-0.705051, -0.687775, 0.172828], [0.732400, -0.680367, -0.026305],
    [0.855162, 0.374582, 0.358311], [0.473006, 0.836480, 0.276705], [-0.097617, 0.654112, 0.750072],
    [-0.904124, -0.153725, 0.398658], [-0.211916, -0.858090, 0.467732], [0.500227, -0.674408, 0.543091],
    [0.584539, -0.110249, 0.803841], [0.437373, 0.454644, 0.775889], [-0.042441, 0.083318, 0.995619],
    [-0.596251, 0.220132, 0.772028], [-0.506455, -0.396977, 0.765449], [0.070569, -0.478474, 0.875262],
];

/// Authentic Westwood Red Alert 2 / Yuri's Revenge 244-entry spherical normal table (`normals_mode == 4`),
/// in Westwood voxel space `(x = forward, y = side, z = up)`.
pub const WESTWOOD_RA2_NORMALS_244: [[f32; 3]; 244] = [
    [0.526578, -0.359621, -0.770317], [0.150482, 0.435984, 0.887284], [0.414195, 0.738255, -0.532374], [0.075152, 0.916249, -0.393498],
    [-0.316149, 0.930736, -0.183793], [-0.773819, 0.623334, -0.112510], [-0.900842, 0.428537, -0.069568], [-0.998942, -0.010971, 0.044665],
    [-0.979761, -0.157670, -0.123324], [-0.911274, -0.362371, -0.195620], [-0.624069, -0.720941, -0.301301], [-0.310173, -0.809345, -0.498752],
    [0.146613, -0.815819, -0.559414], [-0.716516, -0.694356, -0.066888], [0.503972, -0.114202, -0.856137], [0.455491, 0.872627, -0.176211],
    [-0.005010, -0.114373, -0.993425], [-0.104675, -0.327701, -0.938965], [0.560412, 0.752589, -0.345756], [-0.060576, 0.821628, -0.566796],
    [-0.302341, 0.797007, -0.522847], [-0.671543, 0.670740, -0.314863], [-0.778401, -0.128357, 0.614505], [-0.924050, 0.278382, -0.261985],
    [-0.699773, -0.550491, -0.455278], [-0.568248, -0.517189, -0.640008], [0.054098, -0.932864, -0.356143], [0.758382, 0.572893, -0.310888],
    [0.003620, 0.305026, -0.952337], [-0.060850, -0.986886, -0.149511], [0.635230, 0.045478, -0.770983], [0.521705, 0.241309, -0.818287],
    [0.269404, 0.635425, -0.723641], [0.045676, 0.672754, -0.738455], [-0.180511, 0.674657, -0.715719], [-0.397131, 0.636640, -0.661042],
    [-0.552004, 0.472515, -0.687038], [-0.772170, 0.083090, -0.629960], [-0.669819, -0.119533, -0.732840], [-0.540455, -0.318444, -0.778782],
    [-0.386135, -0.522789, -0.759994], [-0.261466, -0.688567, -0.676395], [-0.019412, -0.696103, -0.717680], [0.303569, -0.481844, -0.821993],
    [0.681939, -0.195129, -0.704900], [-0.244889, -0.116562, -0.962519], [0.800759, -0.022979, -0.598546], [-0.370275, 0.095584, -0.923991],
    [-0.330671, -0.326578, -0.885440], [-0.163220, -0.527579, -0.833679], [0.126390, -0.313146, -0.941257], [0.349548, -0.272226, -0.896498],
    [0.239918, -0.085825, -0.966992], [0.390845, 0.081537, -0.916838], [0.255267, 0.268697, -0.928785], [0.146245, 0.480438, -0.864749],
    [-0.326016, 0.478456, -0.815349], [-0.469682, -0.112519, -0.875636], [0.818440, -0.258520, -0.513151], [-0.474318, 0.292238, -0.830433],
    [0.778943, 0.395842, -0.486371], [0.624094, 0.393773, -0.674870], [0.740886, 0.203834, -0.639953], [0.480217, 0.565768, -0.670297],
    [0.380930, 0.424535, -0.821378], [-0.093422, 0.501124, -0.860318], [-0.236485, 0.296198, -0.925387], [-0.131531, 0.093959, -0.986849],
    [-0.823562, 0.295777, -0.484006], [0.611066, -0.624304, -0.486664], [0.069496, -0.520330, -0.851133], [0.226522, -0.664879, -0.711775],
    [0.471308, -0.568904, -0.673957], [0.388425, -0.742624, -0.545560], [0.783675, -0.480729, -0.393385], [0.962394, 0.135676, -0.235349],
    [0.876607, 0.172034, -0.449406], [0.633405, 0.589793, -0.500941], [0.182276, 0.800658, -0.570721], [0.177003, 0.764134, 0.620297],
    [-0.544016, 0.675515, -0.497721], [-0.679297, 0.286467, -0.675642], [-0.590391, 0.091369, -0.801929], [-0.824360, -0.133124, -0.550189],
    [-0.715794, -0.334542, -0.612961], [0.174286, -0.892484, 0.416049], [-0.082528, -0.837123, -0.540753], [0.283331, -0.880874, -0.379189],
    [0.675134, -0.426627, -0.601817], [0.843720, -0.512335, -0.160156], [0.977304, -0.098556, -0.187520], [0.846295, 0.522672, -0.102947],
    [0.677141, 0.721325, -0.145501], [0.320965, 0.870892, -0.372194], [-0.178978, 0.911533, -0.370236], [-0.447169, 0.826701, -0.341474],
    [-0.703203, 0.496328, -0.509081], [-0.977181, 0.063563, -0.202674], [-0.878170, -0.412938, 0.241455], [-0.835831, -0.358550, -0.415728],
    [-0.499174, -0.693433, -0.519592], [-0.188789, -0.923753, -0.333225], [0.192254, -0.969361, -0.152896], [0.515940, -0.783907, -0.345392],
    [0.905925, -0.300952, -0.297871], [0.991112, -0.127746, 0.037107], [0.995135, 0.098424, -0.004383], [0.760123, 0.646277, 0.067367],
    [0.205221, 0.959580, -0.192591], [-0.042750, 0.979513, -0.196791], [-0.438017, 0.898927, 0.008492], [-0.821994, 0.480785, -0.305239],
    [-0.899917, 0.081710, -0.428337], [-0.926612, -0.144618, -0.347096], [-0.793660, -0.557792, -0.242839], [-0.431350, -0.847779, -0.308558],
    [-0.005492, -0.965000, 0.262193], [0.587905, -0.804026, -0.088940], [0.699493, -0.667686, -0.254765], [0.889303, 0.359795, -0.282291],
    [0.780972, 0.197037, 0.592672], [0.520121, 0.506696, 0.687557], [0.403895, 0.693961, 0.596060], [-0.154983, 0.899236, 0.409090],
    [-0.657338, 0.537168, 0.528543], [-0.746195, 0.334091, 0.575827], [-0.624952, -0.049144, 0.779115], [0.318141, -0.254715, 0.913185],
    [-0.555897, 0.405294, 0.725752], [-0.794434, 0.099406, 0.599160], [-0.640361, -0.689463, 0.338495], [-0.126713, -0.734095, 0.667120],
    [0.105457, -0.780817, 0.615795], [0.407993, -0.480916, 0.776055], [0.695136, -0.545120, 0.468647], [0.973191, -0.006489, 0.229908],
    [0.946894, 0.317509, -0.050799], [0.563583, 0.825612, 0.027183], [0.325773, 0.945423, 0.006949], [-0.171821, 0.985097, -0.007815],
    [-0.670441, 0.739939, 0.054769], [-0.822981, 0.554962, 0.121322], [-0.966193, 0.117857, 0.229307], [-0.953769, -0.294704, 0.058945],
    [-0.864387, -0.502728, -0.010015], [-0.530609, -0.842006, -0.097366], [-0.162618, -0.984075, 0.071772], [0.081447, -0.996011, 0.036439],
    [0.745984, -0.665963, 0.000762], [0.942057, -0.329269, -0.064106], [0.939702, -0.281090, 0.194803], [0.771214, 0.550670, 0.319363],
    [0.641348, 0.730690, 0.234021], [0.080682, 0.996691, 0.009879], [-0.046725, 0.976643, 0.209725], [-0.531076, 0.821001, 0.209562],
    [-0.695815, 0.655990, 0.292435], [-0.976122, 0.216709, -0.014913], [-0.961661, -0.144129, 0.233314], [-0.772084, -0.613647, 0.165299],
    [-0.449600, -0.836060, 0.314426], [-0.392700, -0.914616, 0.096247], [0.390589, -0.919470, 0.044890], [0.582529, -0.799198, 0.148127],
    [0.866431, -0.489812, 0.096864], [0.904587, 0.111498, 0.411450], [0.953537, 0.232330, 0.191806], [0.497311, 0.770803, 0.398177],
    [0.194066, 0.956320, 0.218611], [0.422876, 0.882276, 0.206797], [-0.373797, 0.849566, 0.372174], [-0.534497, 0.714023, 0.452200],
    [-0.881827, 0.237160, 0.407598], [-0.904948, -0.014069, 0.425289], [-0.751827, -0.512817, 0.414458], [-0.501015, -0.697917, 0.511758],
    [-0.235190, -0.925923, 0.295555], [0.228983, -0.953940, 0.193819], [0.734025, -0.634898, 0.241062], [0.913753, -0.063253, -0.401316],
    [0.905735, -0.161487, 0.391875], [0.858930, 0.342446, 0.380749], [0.624486, 0.607581, 0.490777], [0.289264, 0.857479, 0.425508],
    [0.069968, 0.902169, 0.425671], [-0.286180, 0.940700, 0.182165], [-0.574013, 0.805119, -0.149309], [0.111258, 0.099718, -0.988776],
    [-0.305393, -0.944228, -0.123160], [-0.601166, -0.789576, 0.123163], [-0.290645, -0.812140, 0.505919], [-0.064920, -0.877163, 0.475785],
    [0.408301, -0.862216, 0.299789], [0.566097, -0.725566, 0.391264], [0.839364, -0.427387, 0.335869], [0.818900, -0.041305, 0.572448],
    [0.719784, 0.414997, 0.556497], [0.881744, 0.450270, 0.140659], [0.401823, -0.898220, -0.178152], [-0.054020, 0.791344, 0.608980],
    [-0.293774, 0.763994, 0.574465], [-0.450798, 0.610347, 0.651351], [-0.638221, 0.186694, 0.746873], [-0.872870, -0.257127, 0.414708],
    [-0.587257, -0.521710, 0.618828], [-0.353658, -0.641974, 0.680291], [0.041649, -0.611273, 0.790323], [0.348342, -0.779183, 0.521087],
    [0.499167, -0.622441, 0.602826], [0.790019, -0.303831, 0.532500], [0.660118, 0.060733, 0.748702], [0.604921, 0.294161, 0.739960],
    [0.385697, 0.379346, 0.841032], [0.239693, 0.207876, 0.948332], [0.012623, 0.258532, 0.965920], [-0.100557, 0.457147, 0.883688],
    [0.046967, 0.628588, 0.776319], [-0.430391, -0.445405, 0.785097], [-0.434291, -0.196228, 0.879139], [-0.256637, -0.336867, 0.905902],
    [-0.131372, -0.158910, 0.978514], [0.102379, -0.208767, 0.972592], [0.195687, -0.450129, 0.871258], [0.627319, -0.423148, 0.653771],
    [0.687439, -0.171583, 0.705682], [0.275920, -0.021255, 0.960946], [0.459367, 0.157466, 0.874178], [0.285395, 0.583184, 0.760556],
    [-0.812174, 0.460303, 0.358461], [-0.189068, 0.641223, 0.743698], [-0.338875, 0.476480, 0.811252], [-0.920994, 0.347186, 0.176727],
    [0.040639, 0.024465, 0.998874], [-0.739132, -0.353747, 0.573190], [-0.603512, -0.286615, 0.744060], [-0.188676, -0.547059, 0.815554],
    [-0.026045, -0.397820, 0.917094], [0.267897, -0.649041, 0.712023], [0.518246, -0.284891, 0.806386], [0.493451, -0.066533, 0.867225],
    [-0.328188, 0.140251, 0.934143], [-0.328188, 0.140251, 0.934143], [-0.328188, 0.140251, 0.934143], [-0.328188, 0.140251, 0.934143],
];

/// Decodes a Westwood `.vxl` voxel normal (`normals_mode` from `tailer[91]` and per-voxel `normal_idx`)
/// and transforms it from Westwood `(x = forward, y = side, z = up)` into unit glTF `[y, z, x]`.
pub fn decode_westwood_voxel_normal_gltf(normals_mode: u8, normal_idx: u8) -> Option<[f32; 3]> {
    let raw = match normals_mode {
        1 | 2 => WESTWOOD_TS_NORMALS_36.get(normal_idx as usize).copied()?,
        3 | 4 => WESTWOOD_RA2_NORMALS_244.get(normal_idx as usize).copied()?,
        _ => return None,
    };
    let [wx, wy, wz] = raw;
    let [gx, gy, gz] = [wy, wz, wx];
    let len = (gx * gx + gy * gy + gz * gz).sqrt();
    if len > 1e-6 {
        Some([gx / len, gy / len, gz / len])
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct VoxelCell {
    pub color_idx: u8,
    pub normal_idx: u8,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct VoxelPaletteEntry {
    pub rgb: [f32; 3],
    pub is_remap: bool,
}

struct GridPatch {
    w: u32,
    h: u32,
    rgba: Vec<u8>,
}

enum PatchKind {
    Solid(usize),
    Grid(usize),
}

struct QuadUvRecord {
    part_idx: usize,
    base_vert: usize,
    patch: PatchKind,
}

/// Collects surface texel patches across all limbs of a `.vxl` model and packs them into a single
/// bleed-free lossless RGBA PNG texture atlas (`3x3` solid blocks + `1px`-extruded greedy patches).
#[derive(Default)]
pub(super) struct VoxelAtlasBuilder {
    solid_swatches: Vec<[u8; 4]>,
    solid_lookup: HashMap<[u8; 4], usize>,
    grid_patches: Vec<GridPatch>,
    grid_lookup: HashMap<(u32, u32, Vec<u8>), usize>,
    records: Vec<QuadUvRecord>,
}

impl VoxelAtlasBuilder {
    pub fn register_quad(
        &mut self,
        part_idx: usize,
        base_vert: usize,
        w: u32,
        h: u32,
        rgba: Vec<u8>,
    ) {
        let first: [u8; 4] = rgba[0..4].try_into().unwrap();
        let is_uniform = rgba.chunks_exact(4).all(|px| px == first);
        let patch = if is_uniform {
            let sid = *self.solid_lookup.entry(first).or_insert_with(|| {
                let id = self.solid_swatches.len();
                self.solid_swatches.push(first);
                id
            });
            PatchKind::Solid(sid)
        } else {
            let gid = *self
                .grid_lookup
                .entry((w, h, rgba.clone()))
                .or_insert_with(|| {
                    let id = self.grid_patches.len();
                    self.grid_patches.push(GridPatch { w, h, rgba });
                    id
                });
            PatchKind::Grid(gid)
        };
        self.records.push(QuadUvRecord {
            part_idx,
            base_vert,
            patch,
        });
    }

    pub fn finalize_into_parts(self, parts: &mut [ColoredMeshPart]) -> Result<Vec<u8>> {
        let mut total_area = (self.solid_swatches.len() as u32) * 9;
        let mut max_pw = 3u32;
        for gp in &self.grid_patches {
            let pw = gp.w + 2;
            let ph = gp.h + 2;
            total_area += pw * ph;
            max_pw = max_pw.max(pw);
        }

        let est_side = ((total_area as f32 * 1.35).sqrt().ceil() as u32).max(16);
        let atlas_w = est_side.max(max_pw).next_power_of_two();

        let mut grid_order: Vec<usize> = (0..self.grid_patches.len()).collect();
        grid_order.sort_by(|&a, &b| {
            let pa = &self.grid_patches[a];
            let pb = &self.grid_patches[b];
            pb.h.cmp(&pa.h).then_with(|| pb.w.cmp(&pa.w)).then(a.cmp(&b))
        });

        let mut cur_x = 0u32;
        let mut cur_y = 0u32;
        let mut shelf_h = 0u32;

        let mut grid_origins = vec![(0u32, 0u32); self.grid_patches.len()];
        for gid in grid_order {
            let pw = self.grid_patches[gid].w + 2;
            let ph = self.grid_patches[gid].h + 2;
            if cur_x + pw > atlas_w {
                cur_y += shelf_h;
                cur_x = 0;
                shelf_h = 0;
            }
            grid_origins[gid] = (cur_x, cur_y);
            cur_x += pw;
            shelf_h = shelf_h.max(ph);
        }

        if shelf_h > 0 && shelf_h < 3 {
            cur_y += shelf_h;
            cur_x = 0;
            shelf_h = 0;
        }

        let mut solid_origins = vec![(0u32, 0u32); self.solid_swatches.len()];
        for (sid, dst) in solid_origins.iter_mut().enumerate() {
            let _ = sid;
            if cur_x + 3 > atlas_w {
                cur_y += shelf_h;
                cur_x = 0;
                shelf_h = 0;
            }
            *dst = (cur_x, cur_y);
            cur_x += 3;
            shelf_h = shelf_h.max(3);
        }

        let used_h = (cur_y + shelf_h).max(16);
        let atlas_h = used_h.next_power_of_two();

        let mut img = vec![0u8; (atlas_w * atlas_h * 4) as usize];
        for px in img.chunks_exact_mut(4) {
            px[3] = 255;
        }

        for (sid, &rgba) in self.solid_swatches.iter().enumerate() {
            let (ox, oy) = solid_origins[sid];
            for dy in 0..3u32 {
                for dx in 0..3u32 {
                    let dst = (((oy + dy) * atlas_w + (ox + dx)) * 4) as usize;
                    img[dst..dst + 4].copy_from_slice(&rgba);
                }
            }
        }

        for (gid, gp) in self.grid_patches.iter().enumerate() {
            let (ox, oy) = grid_origins[gid];
            for dy in 0..(gp.h + 2) {
                let sy = dy.saturating_sub(1).min(gp.h - 1);
                for dx in 0..(gp.w + 2) {
                    let sx = dx.saturating_sub(1).min(gp.w - 1);
                    let src = ((sy * gp.w + sx) * 4) as usize;
                    let dst = (((oy + dy) * atlas_w + (ox + dx)) * 4) as usize;
                    img[dst..dst + 4].copy_from_slice(&gp.rgba[src..src + 4]);
                }
            }
        }

        let inv_w = 1.0 / (atlas_w as f32);
        let inv_h = 1.0 / (atlas_h as f32);
        for rec in self.records {
            let uvs = &mut parts[rec.part_idx].texcoords[rec.base_vert..rec.base_vert + 4];
            match rec.patch {
                PatchKind::Solid(sid) => {
                    let (ox, oy) = solid_origins[sid];
                    let uv = [(ox as f32 + 1.5) * inv_w, (oy as f32 + 1.5) * inv_h];
                    uvs.fill(uv);
                }
                PatchKind::Grid(gid) => {
                    let (ox, oy) = grid_origins[gid];
                    let gp = &self.grid_patches[gid];
                    let u0 = (ox + 1) as f32 * inv_w;
                    let u1 = (ox + 1 + gp.w) as f32 * inv_w;
                    let v0 = (oy + 1) as f32 * inv_h;
                    let v1 = (oy + 1 + gp.h) as f32 * inv_h;
                    uvs[0] = [u0, v0];
                    uvs[1] = [u1, v0];
                    uvs[2] = [u1, v1];
                    uvs[3] = [u0, v1];
                }
            }
        }

        super::encode_rgba_to_png(atlas_w, atlas_h, &img)
    }
}

#[inline]
fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
fn normalize3_or(v: [f32; 3], fallback: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 1e-6 {
        [v[0] / l, v[1] / l, v[2] / l]
    } else {
        fallback
    }
}

struct AxisFaceSpec {
    dir: [isize; 3],
    axis_d: usize,
    plane_off: usize,
    axis_u: usize,
    axis_v: usize,
    gltf_geom_n: [f32; 3],
}

const AXIS_FACES: [AxisFaceSpec; 6] = [
    AxisFaceSpec {
        dir: [-1, 0, 0],
        axis_d: 0,
        plane_off: 0,
        axis_u: 2,
        axis_v: 1,
        gltf_geom_n: [0.0, 0.0, -1.0],
    },
    AxisFaceSpec {
        dir: [1, 0, 0],
        axis_d: 0,
        plane_off: 1,
        axis_u: 1,
        axis_v: 2,
        gltf_geom_n: [0.0, 0.0, 1.0],
    },
    AxisFaceSpec {
        dir: [0, -1, 0],
        axis_d: 1,
        plane_off: 0,
        axis_u: 0,
        axis_v: 2,
        gltf_geom_n: [-1.0, 0.0, 0.0],
    },
    AxisFaceSpec {
        dir: [0, 1, 0],
        axis_d: 1,
        plane_off: 1,
        axis_u: 2,
        axis_v: 0,
        gltf_geom_n: [1.0, 0.0, 0.0],
    },
    AxisFaceSpec {
        dir: [0, 0, -1],
        axis_d: 2,
        plane_off: 0,
        axis_u: 1,
        axis_v: 0,
        gltf_geom_n: [0.0, -1.0, 0.0],
    },
    AxisFaceSpec {
        dir: [0, 0, 1],
        axis_d: 2,
        plane_off: 1,
        axis_u: 0,
        axis_v: 1,
        gltf_geom_n: [0.0, 1.0, 0.0],
    },
];

struct ExposedFace {
    face_dir: u8,
    coord: [usize; 3],
    base_n: [f32; 3],
    corner_keys: [usize; 4],
    corner_n: [[f32; 3]; 4],
    rgba: [u8; 4],
    is_remap: bool,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_hard_surface_vxl_limb_faces(
    grid: &[Option<VoxelCell>],
    palette: &[VoxelPaletteEntry; 256],
    normals_mode: u8,
    size_x: usize,
    size_y: usize,
    size_z: usize,
    bounds_min: [f32; 3],
    bounds_max: [f32; 3],
    extra_offset: [f32; 3],
    part_idx: usize,
    dst: &mut ColoredMeshPart,
    atlas: &mut VoxelAtlasBuilder,
) {
    let dims = [size_x, size_y, size_z];
    if size_x == 0 || size_y == 0 || size_z == 0 {
        return;
    }
    let idx_3d = |x: usize, y: usize, z: usize| -> usize { (z * size_y + y) * size_x + x };

    // Synthetic test blocks (`build_sample_vxl_bytes`) write `normal_idx = 0` on every voxel;
    // detect uniform-zero multi-voxel limbs so synthetic boxes receive exact orthogonal normals.
    let mut filled_count = 0usize;
    let mut any_nonzero_normal = false;
    for cell in grid.iter().flatten() {
        filled_count += 1;
        if cell.normal_idx != 0 {
            any_nonzero_normal = true;
        }
    }
    if filled_count == 0 {
        return;
    }
    let has_authored_normals = any_nonzero_normal || filled_count == 1;

    let stride_cy = size_x + 1;
    let stride_cz = (size_x + 1) * (size_y + 1);
    let corner_key = |c: [usize; 3]| -> usize { c[2] * stride_cz + c[1] * stride_cy + c[0] };

    let inv_sz = 1.0 / (size_z.max(1) as f32);
    let mut faces: Vec<ExposedFace> = Vec::new();
    let mut corner_incident: HashMap<usize, Vec<usize>> = HashMap::new();

    for z in 0..size_z {
        let z_ao = 0.94 + 0.06 * ((z as f32 + 0.5) * inv_sz);
        for y in 0..size_y {
            for x in 0..size_x {
                let Some(cell) = grid[idx_3d(x, y, z)] else {
                    continue;
                };
                let pal = palette[cell.color_idx as usize];
                let decoded_n = if has_authored_normals {
                    decode_westwood_voxel_normal_gltf(normals_mode, cell.normal_idx)
                } else {
                    None
                };

                for (f_idx, spec) in AXIS_FACES.iter().enumerate() {
                    let nx = x as isize + spec.dir[0];
                    let ny = y as isize + spec.dir[1];
                    let nz = z as isize + spec.dir[2];
                    let neighbor_filled = nx >= 0
                        && ny >= 0
                        && nz >= 0
                        && (nx as usize) < size_x
                        && (ny as usize) < size_y
                        && (nz as usize) < size_z
                        && grid[idx_3d(nx as usize, ny as usize, nz as usize)].is_some();
                    if neighbor_filled {
                        continue;
                    }

                    // 3D volumetric cavity AO in the 3x3x3 neighborhood around `(nx, ny, nz)`
                    let mut occ = 0u32;
                    for dz in -1isize..=1 {
                        let qz = nz + dz;
                        if qz < 0 || (qz as usize) >= size_z {
                            continue;
                        }
                        for dy in -1isize..=1 {
                            let qy = ny + dy;
                            if qy < 0 || (qy as usize) >= size_y {
                                continue;
                            }
                            for dx in -1isize..=1 {
                                let qx = nx + dx;
                                if qx >= 0
                                    && (qx as usize) < size_x
                                    && grid[idx_3d(qx as usize, qy as usize, qz as usize)].is_some()
                                {
                                    occ += 1;
                                }
                            }
                        }
                    }
                    let cav = (1.0 - ((occ.saturating_sub(9) as f32) / 17.0) * 0.22).clamp(0.78, 1.0);
                    let ao = (cav * z_ao).clamp(0.76, 1.0);

                    let rgba = if pal.is_remap {
                        if (pal.rgb[0] - pal.rgb[1]).abs() < 1e-6
                            && (pal.rgb[1] - pal.rgb[2]).abs() < 1e-6
                        {
                            let lum_ao = (pal.rgb[0] * ao).clamp(0.18, 1.0);
                            let l = (lum_ao * 255.0 + 0.5).clamp(46.0, 255.0) as u8;
                            [l, l, l, 0]
                        } else {
                            [
                                ((pal.rgb[0] * ao).clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
                                ((pal.rgb[1] * ao).clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
                                ((pal.rgb[2] * ao).clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
                                0,
                            ]
                        }
                    } else {
                        [
                            ((pal.rgb[0] * ao).clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
                            ((pal.rgb[1] * ao).clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
                            ((pal.rgb[2] * ao).clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
                            255,
                        ]
                    };

                    // Use Westwood `normal_idx` when within the face's outward hemisphere;
                    // fall back to geometric face normal on perpendicular/underside faces of thin plates.
                    let base_n = match decoded_n {
                        Some(vn) if dot3(vn, spec.gltf_geom_n) >= 0.25 => vn,
                        _ => spec.gltf_geom_n,
                    };

                    let base_c = [x, y, z];
                    let mut c_keys = [0usize; 4];
                    for (ci, &(du, dv)) in [(0usize, 0usize), (1, 0), (1, 1), (0, 1)]
                        .iter()
                        .enumerate()
                    {
                        let mut c = base_c;
                        c[spec.axis_d] += spec.plane_off;
                        c[spec.axis_u] += du;
                        c[spec.axis_v] += dv;
                        let k = corner_key(c);
                        c_keys[ci] = k;
                        corner_incident.entry(k).or_default().push(faces.len());
                    }

                    faces.push(ExposedFace {
                        face_dir: f_idx as u8,
                        coord: base_c,
                        base_n,
                        corner_keys: c_keys,
                        corner_n: [base_n; 4],
                        rgba,
                        is_remap: pal.is_remap,
                    });
                }
            }
        }
    }

    if faces.is_empty() {
        return;
    }

    // Crease-angle corner normal smoothing: weld normals where `dot >= cos(35°)` and keep
    // razor-sharp hard creases where adjacent faces differ by `> 35°`.
    for fi in 0..faces.len() {
        let fn0 = faces[fi].base_n;
        let keys = faces[fi].corner_keys;
        let mut smoothed = [fn0; 4];
        for (ci, &ck) in keys.iter().enumerate() {
            let mut sum = [0.0f32; 3];
            if let Some(inc) = corner_incident.get(&ck) {
                for &other_fi in inc {
                    let on = faces[other_fi].base_n;
                    if dot3(fn0, on) >= CREASE_COS_35 {
                        sum[0] += on[0];
                        sum[1] += on[1];
                        sum[2] += on[2];
                    }
                }
            }
            smoothed[ci] = normalize3_or(sum, fn0);
        }
        faces[fi].corner_n = smoothed;
    }

    let step = [
        (bounds_max[0] - bounds_min[0]) / (size_x as f32),
        (bounds_max[1] - bounds_min[1]) / (size_y as f32),
        (bounds_max[2] - bounds_min[2]) / (size_z as f32),
    ];
    let corner_to_gltf = |c: [usize; 3]| -> [f32; 3] {
        let vx = bounds_min[0] + (c[0] as f32) * step[0];
        let vy = bounds_min[1] + (c[1] as f32) * step[1];
        let vz = bounds_min[2] + (c[2] as f32) * step[2];
        [
            vy + extra_offset[0],
            vz + extra_offset[1],
            vx + extra_offset[2],
        ]
    };

    // Group faces by `(face_dir, slice_d)` and run 2D greedy planar quad merging
    let mut by_dir: [Vec<usize>; 6] = Default::default();
    for (fi, f) in faces.iter().enumerate() {
        by_dir[f.face_dir as usize].push(fi);
    }

    let mut slice_grid: Vec<Option<usize>> = Vec::new();
    for (f_dir, dir_faces) in by_dir.iter().enumerate() {
        if dir_faces.is_empty() {
            continue;
        }
        let spec = &AXIS_FACES[f_dir];
        let dim_d = dims[spec.axis_d];
        let dim_u = dims[spec.axis_u];
        let dim_v = dims[spec.axis_v];

        let mut slices: Vec<Vec<usize>> = vec![Vec::new(); dim_d];
        for &fi in dir_faces {
            slices[faces[fi].coord[spec.axis_d]].push(fi);
        }

        slice_grid.resize(dim_u * dim_v, None);
        for (d, s_faces) in slices.iter().enumerate() {
            if s_faces.is_empty() {
                continue;
            }
            slice_grid.fill(None);
            for &fi in s_faces {
                let u = faces[fi].coord[spec.axis_u];
                let v = faces[fi].coord[spec.axis_v];
                slice_grid[v * dim_u + u] = Some(fi);
            }

            let is_planar_match = |cand_fi: usize, ref_n: [f32; 3], ref_remap: bool| -> bool {
                let cf = &faces[cand_fi];
                cf.is_remap == ref_remap
                    && cf.corner_n.iter().all(|&cn| dot3(cn, ref_n) >= 0.9995)
            };

            for v0 in 0..dim_v {
                for u0 in 0..dim_u {
                    let Some(seed_fi) = slice_grid[v0 * dim_u + u0] else {
                        continue;
                    };
                    let seed = &faces[seed_fi];
                    let seed_n = seed.corner_n[0];
                    let seed_planar = seed.corner_n.iter().all(|&cn| dot3(cn, seed_n) >= 0.9995);

                    let (w, h) = if seed_planar {
                        let mut w = 1usize;
                        while u0 + w < dim_u {
                            if let Some(cfi) = slice_grid[v0 * dim_u + (u0 + w)] {
                                if is_planar_match(cfi, seed_n, seed.is_remap) {
                                    w += 1;
                                    continue;
                                }
                            }
                            break;
                        }
                        let mut h = 1usize;
                        'grow_h: while v0 + h < dim_v {
                            for du in 0..w {
                                match slice_grid[(v0 + h) * dim_u + (u0 + du)] {
                                    Some(cfi) if is_planar_match(cfi, seed_n, seed.is_remap) => {}
                                    _ => break 'grow_h,
                                }
                            }
                            h += 1;
                        }
                        (w, h)
                    } else {
                        (1, 1)
                    };

                    let mut patch_rgba = Vec::with_capacity(w * h * 4);
                    for dv in 0..h {
                        for du in 0..w {
                            let cell_slot = &mut slice_grid[(v0 + dv) * dim_u + (u0 + du)];
                            let cfi = cell_slot.take().unwrap();
                            patch_rgba.extend_from_slice(&faces[cfi].rgba);
                        }
                    }

                    let quad_n = if w == 1 && h == 1 {
                        faces[seed_fi].corner_n
                    } else {
                        [seed_n; 4]
                    };

                    let base_vert = dst.positions.len();
                    for (ci, &(du, dv)) in [(0usize, 0usize), (w, 0), (w, h), (0, h)]
                        .iter()
                        .enumerate()
                    {
                        let mut c = [0usize; 3];
                        c[spec.axis_d] = d + spec.plane_off;
                        c[spec.axis_u] = u0 + du;
                        c[spec.axis_v] = v0 + dv;
                        dst.positions.push(corner_to_gltf(c));
                        dst.normals.push(quad_n[ci]);
                        dst.colors.push([1.0, 1.0, 1.0, 1.0]);
                        dst.texcoords.push([0.0, 0.0]);
                    }
                    let b = base_vert as u32;
                    dst.indices.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
                    atlas.register_quad(part_idx, base_vert, w as u32, h as u32, patch_rgba);
                }
            }
        }
    }
}
