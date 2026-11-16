use crate::glb_optimizer::parse_glb_chunks;
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::io::Cursor;

const COMP_BYTE: u32 = 5120;
const COMP_UBYTE: u32 = 5121;
const COMP_SHORT: u32 = 5122;
const COMP_USHORT: u32 = 5123;
const COMP_UINT: u32 = 5125;
const COMP_FLOAT: u32 = 5126;

#[derive(Clone)]
pub(super) struct DecodedTex {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl DecodedTex {
    #[inline]
    pub fn sample_bilinear(&self, uv: [f32; 2]) -> [f32; 4] {
        if self.width == 0 || self.height == 0 || self.rgba.len() < 4 {
            return [1.0, 1.0, 1.0, 1.0];
        }
        let u = uv[0].rem_euclid(1.0);
        let v = uv[1].rem_euclid(1.0);
        let (w, h) = (self.width as i32, self.height as i32);
        let (gx, gy) = (u * w as f32 - 0.5, v * h as f32 - 0.5);
        let (x0, y0) = (gx.floor() as i32, gy.floor() as i32);
        let (fx, fy) = (gx - x0 as f32, gy - y0 as f32);
        let (ix0, ix1) = (x0.rem_euclid(w) as usize, (x0 + 1).rem_euclid(w) as usize);
        let iy0 = y0.rem_euclid(h) as usize * (w as usize);
        let iy1 = (y0 + 1).rem_euclid(h) as usize * (w as usize);
        let (p00, p10, p01, p11) = (
            (iy0 + ix0) * 4,
            (iy0 + ix1) * 4,
            (iy1 + ix0) * 4,
            (iy1 + ix1) * 4,
        );
        let mut out = [0.0f32; 4];
        for (c, item) in out.iter_mut().enumerate() {
            let top = self.rgba[p00 + c] as f32
                + (self.rgba[p10 + c] as f32 - self.rgba[p00 + c] as f32) * fx;
            let bot = self.rgba[p01 + c] as f32
                + (self.rgba[p11 + c] as f32 - self.rgba[p01 + c] as f32) * fx;
            *item = (top + (bot - top) * fy) * (1.0 / 255.0);
        }
        out
    }
}

#[derive(Clone)]
pub(super) struct PbrMaterialSpec {
    pub base_factor: [f32; 4],
    pub base_tex: Option<usize>,
    pub mr_tex: Option<usize>,
    pub occ_tex: Option<usize>,
    pub emissive_tex: Option<usize>,
    pub emissive_factor: [f32; 3],
    pub metallic_factor: f32,
    pub roughness_factor: f32,
    pub double_sided: bool,
    pub is_primary_slot: bool,
}

impl Default for PbrMaterialSpec {
    fn default() -> Self {
        Self {
            base_factor: [1.0, 1.0, 1.0, 1.0],
            base_tex: None,
            mr_tex: None,
            occ_tex: None,
            emissive_tex: None,
            emissive_factor: [0.0, 0.0, 0.0],
            metallic_factor: 0.20,
            roughness_factor: 0.62,
            double_sided: false,
            is_primary_slot: false,
        }
    }
}

pub(super) struct TexturedTri {
    pub p: [[f32; 3]; 3],
    pub n: [[f32; 3]; 3],
    pub uv: [[f32; 2]; 3],
    pub vc: [[f32; 4]; 3],
    pub mat_idx: usize,
}

#[inline]
pub(super) fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 1e-6 {
        [v[0] / l, v[1] / l, v[2] / l]
    } else {
        [0.0, 1.0, 0.0]
    }
}

fn mat4_identity() -> [f32; 16] {
    let mut m = [0.0f32; 16];
    m[0] = 1.0;
    m[5] = 1.0;
    m[10] = 1.0;
    m[15] = 1.0;
    m
}

fn mat4_mul(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut out = [0.0f32; 16];
    for col in 0..4 {
        for row in 0..4 {
            out[col * 4 + row] = (0..4).map(|k| a[k * 4 + row] * b[col * 4 + k]).sum();
        }
    }
    out
}

fn parse_node_mat4(node: &Value) -> [f32; 16] {
    if let Some(arr) = node
        .get("matrix")
        .and_then(Value::as_array)
        .filter(|a| a.len() == 16)
    {
        let mut m = [0.0f32; 16];
        for (i, v) in arr.iter().enumerate() {
            m[i] = v.as_f64().unwrap_or(0.0) as f32;
        }
        return m;
    }
    let f3 = |key: &str, def: [f32; 3]| -> [f32; 3] {
        node.get(key)
            .and_then(Value::as_array)
            .filter(|a| a.len() >= 3)
            .map(|a| {
                [
                    a[0].as_f64().unwrap_or(def[0] as f64) as f32,
                    a[1].as_f64().unwrap_or(def[1] as f64) as f32,
                    a[2].as_f64().unwrap_or(def[2] as f64) as f32,
                ]
            })
            .unwrap_or(def)
    };
    let t = f3("translation", [0.0, 0.0, 0.0]);
    let s = f3("scale", [1.0, 1.0, 1.0]);
    let [x, y, z, w] = node
        .get("rotation")
        .and_then(Value::as_array)
        .filter(|a| a.len() >= 4)
        .map(|a| {
            [
                a[0].as_f64().unwrap_or(0.0) as f32,
                a[1].as_f64().unwrap_or(0.0) as f32,
                a[2].as_f64().unwrap_or(0.0) as f32,
                a[3].as_f64().unwrap_or(1.0) as f32,
            ]
        })
        .unwrap_or([0.0, 0.0, 0.0, 1.0]);
    let (x2, y2, z2) = (x + x, y + y, z + z);
    let (xx, xy, xz) = (x * x2, x * y2, x * z2);
    let (yy, yz, zz) = (y * y2, y * z2, z * z2);
    let (wx, wy, wz) = (w * x2, w * y2, w * z2);
    let mut m = [0.0f32; 16];
    m[0] = (1.0 - (yy + zz)) * s[0];
    m[1] = (xy + wz) * s[0];
    m[2] = (xz - wy) * s[0];
    m[4] = (xy - wz) * s[1];
    m[5] = (1.0 - (xx + zz)) * s[1];
    m[6] = (yz + wx) * s[1];
    m[8] = (xz + wy) * s[2];
    m[9] = (yz - wx) * s[2];
    m[10] = (1.0 - (xx + yy)) * s[2];
    m[12] = t[0];
    m[13] = t[1];
    m[14] = t[2];
    m[15] = 1.0;
    m
}

fn compute_world_mats(json: &Value) -> Vec<[f32; 16]> {
    let id = mat4_identity();
    let Some(nodes) = json.get("nodes").and_then(Value::as_array) else {
        return Vec::new();
    };
    let n = nodes.len();
    let mut world = vec![id; n];
    let mut has_parent = vec![false; n];
    for nd in nodes {
        if let Some(ch) = nd.get("children").and_then(Value::as_array) {
            for c in ch.iter().filter_map(Value::as_u64) {
                if (c as usize) < n {
                    has_parent[c as usize] = true;
                }
            }
        }
    }
    let mut stack: Vec<(usize, [f32; 16])> = (0..n)
        .filter(|&i| !has_parent[i])
        .rev()
        .map(|r| (r, id))
        .collect();
    let mut visited = vec![false; n];
    while let Some((curr, pm)) = stack.pop() {
        if visited[curr] {
            continue;
        }
        visited[curr] = true;
        let wm = mat4_mul(&pm, &parse_node_mat4(&nodes[curr]));
        world[curr] = wm;
        if let Some(ch) = nodes[curr].get("children").and_then(Value::as_array) {
            for c in ch.iter().rev().filter_map(Value::as_u64) {
                if (c as usize) < n {
                    stack.push((c as usize, wm));
                }
            }
        }
    }
    world
}

fn decode_glb_images(json: &Value, bin: &[u8]) -> Vec<Option<DecodedTex>> {
    let Some(images) = json.get("images").and_then(Value::as_array) else {
        return Vec::new();
    };
    let Some(bvs) = json.get("bufferViews").and_then(Value::as_array) else {
        return vec![None; images.len()];
    };
    images
        .iter()
        .map(|img| {
            let bv = bvs.get(img.get("bufferView")?.as_u64()? as usize)?;
            let off = bv.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
            let len = bv.get("byteLength").and_then(Value::as_u64).unwrap_or(0) as usize;
            if len == 0 || off + len > bin.len() {
                return None;
            }
            let mut dec = png::Decoder::new(Cursor::new(&bin[off..off + len]));
            dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
            let mut reader = dec.read_info().ok()?;
            let mut buf = vec![0u8; reader.output_buffer_size()?];
            let info = reader.next_frame(&mut buf).ok()?;
            let raw = &buf[..info.buffer_size()];
            let px = (info.width * info.height) as usize;
            let rgba = match info.color_type {
                png::ColorType::Rgba => raw.get(..px * 4)?.to_vec(),
                png::ColorType::Rgb => raw
                    .get(..px * 3)?
                    .chunks_exact(3)
                    .flat_map(|c| [c[0], c[1], c[2], 255])
                    .collect(),
                png::ColorType::GrayscaleAlpha => raw
                    .get(..px * 2)?
                    .chunks_exact(2)
                    .flat_map(|c| [c[0], c[0], c[0], c[1]])
                    .collect(),
                png::ColorType::Grayscale => raw
                    .get(..px)?
                    .iter()
                    .flat_map(|&g| [g, g, g, 255])
                    .collect(),
                _ => return None,
            };
            Some(DecodedTex {
                width: info.width,
                height: info.height,
                rgba,
            })
        })
        .collect()
}

fn read_f32_accessor(json: &Value, bin: &[u8], acc_idx: usize) -> Result<(usize, Vec<f32>)> {
    let acc = json
        .get("accessors")
        .and_then(Value::as_array)
        .and_then(|a| a.get(acc_idx))
        .context("Missing accessor")?;
    let bv_i = acc
        .get("bufferView")
        .and_then(Value::as_u64)
        .context("Missing bufferView")? as usize;
    let bv = json
        .get("bufferViews")
        .and_then(Value::as_array)
        .and_then(|b| b.get(bv_i))
        .context("Missing bufferView")?;
    let comp = acc
        .get("componentType")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    let ncomp = match acc.get("type").and_then(Value::as_str).unwrap_or("") {
        "SCALAR" => 1usize,
        "VEC2" => 2,
        "VEC3" => 3,
        "VEC4" => 4,
        _ => bail!("Unsupported accessor type"),
    };
    let cs = match comp {
        COMP_BYTE | COMP_UBYTE => 1usize,
        COMP_SHORT | COMP_USHORT => 2,
        COMP_UINT | COMP_FLOAT => 4,
        _ => bail!("Unsupported componentType {comp}"),
    };
    let count = acc.get("count").and_then(Value::as_u64).unwrap_or(0) as usize;
    let start = bv.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize
        + acc.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let stride = bv
        .get("byteStride")
        .and_then(Value::as_u64)
        .map(|s| s as usize)
        .unwrap_or(cs * ncomp);
    let mut out = Vec::with_capacity(count * ncomp);
    for i in 0..count {
        let base = start + i * stride;
        for c in 0..ncomp {
            let s = &bin[base + c * cs..base + (c + 1) * cs];
            out.push(match comp {
                COMP_FLOAT => f32::from_le_bytes(s.try_into().unwrap()),
                COMP_UBYTE => s[0] as f32 / 255.0,
                COMP_USHORT => u16::from_le_bytes(s.try_into().unwrap()) as f32 / 65535.0,
                COMP_BYTE => (s[0] as i8 as f32 / 127.0).max(-1.0),
                COMP_SHORT => {
                    (i16::from_le_bytes(s.try_into().unwrap()) as f32 / 32767.0).max(-1.0)
                }
                COMP_UINT => u32::from_le_bytes(s.try_into().unwrap()) as f32,
                _ => 0.0,
            });
        }
    }
    Ok((ncomp, out))
}

fn read_u32_indices(json: &Value, bin: &[u8], acc_idx: usize) -> Result<Vec<u32>> {
    let acc = &json["accessors"][acc_idx];
    let bv = &json["bufferViews"][acc["bufferView"].as_u64().unwrap_or(0) as usize];
    let comp = acc["componentType"].as_u64().unwrap_or(0) as u32;
    let cs = match comp {
        COMP_UBYTE => 1usize,
        COMP_USHORT => 2,
        COMP_UINT => 4,
        _ => bail!("Unsupported index type"),
    };
    let count = acc["count"].as_u64().unwrap_or(0) as usize;
    let start = bv.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize
        + acc.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let stride = bv
        .get("byteStride")
        .and_then(Value::as_u64)
        .map(|s| s as usize)
        .unwrap_or(cs);
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let s = &bin[start + i * stride..start + i * stride + cs];
        out.push(match comp {
            COMP_UBYTE => s[0] as u32,
            COMP_USHORT => u16::from_le_bytes(s.try_into().unwrap()) as u32,
            _ => u32::from_le_bytes(s.try_into().unwrap()),
        });
    }
    Ok(out)
}

pub(super) type ExtractedTexturedScene = (
    Vec<Option<DecodedTex>>,
    Vec<PbrMaterialSpec>,
    Vec<TexturedTri>,
);

pub(super) fn extract_textured_scene(glb_bytes: &[u8]) -> Result<ExtractedTexturedScene> {
    let (json, bin) = parse_glb_chunks(glb_bytes)?;
    let world_mats = compute_world_mats(&json);
    let images = decode_glb_images(&json, bin);
    let empty = Vec::new();
    let textures = json
        .get("textures")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    let resolve_img = |tex_obj: Option<&Value>| -> Option<usize> {
        let ti = tex_obj?.get("index")?.as_u64()? as usize;
        let si = textures.get(ti)?.get("source")?.as_u64()? as usize;
        images.get(si)?.as_ref().map(|_| si)
    };

    let mut mats = vec![PbrMaterialSpec::default()];
    if let Some(m_arr) = json.get("materials").and_then(Value::as_array) {
        for (mi, m) in m_arr.iter().enumerate() {
            let mut spec = PbrMaterialSpec::default();
            let mname = m.get("name").and_then(Value::as_str).unwrap_or("");
            spec.is_primary_slot =
                mi == 0 && (mname.contains("Primary") || mname.eq_ignore_ascii_case("primary"));
            spec.double_sided = m
                .get("doubleSided")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if let Some(em) = m
                .get("emissiveFactor")
                .and_then(Value::as_array)
                .filter(|a| a.len() >= 3)
            {
                spec.emissive_factor = [
                    em[0].as_f64().unwrap_or(0.0) as f32,
                    em[1].as_f64().unwrap_or(0.0) as f32,
                    em[2].as_f64().unwrap_or(0.0) as f32,
                ];
            }
            spec.emissive_tex = resolve_img(m.get("emissiveTexture"));
            spec.occ_tex = resolve_img(m.get("occlusionTexture"));
            if let Some(pbr) = m.get("pbrMetallicRoughness") {
                if let Some(bcf) = pbr
                    .get("baseColorFactor")
                    .and_then(Value::as_array)
                    .filter(|a| a.len() >= 4)
                {
                    spec.base_factor = [
                        bcf[0].as_f64().unwrap_or(1.0) as f32,
                        bcf[1].as_f64().unwrap_or(1.0) as f32,
                        bcf[2].as_f64().unwrap_or(1.0) as f32,
                        bcf[3].as_f64().unwrap_or(1.0) as f32,
                    ];
                }
                spec.base_tex = resolve_img(pbr.get("baseColorTexture"));
                spec.mr_tex = resolve_img(pbr.get("metallicRoughnessTexture"));
                if let Some(mf) = pbr.get("metallicFactor").and_then(Value::as_f64) {
                    spec.metallic_factor = mf as f32;
                }
                if let Some(rf) = pbr.get("roughnessFactor").and_then(Value::as_f64) {
                    spec.roughness_factor = rf as f32;
                }
            }
            mats.push(spec);
        }
    }

    let nodes = json
        .get("nodes")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    let meshes = json
        .get("meshes")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    let mut tris = Vec::new();
    let id_mat = mat4_identity();

    for (ni, node) in nodes.iter().enumerate() {
        let name = node.get("name").and_then(Value::as_str).unwrap_or("");
        if name.ends_with("_LOD1")
            || name.ends_with("_lod1")
            || name == "ScaffoldFrame"
            || name == "DamageOverlay"
        {
            continue;
        }
        let Some(mi) = node.get("mesh").and_then(Value::as_u64).map(|v| v as usize) else {
            continue;
        };
        let Some(mesh) = meshes.get(mi) else { continue };
        let mname = mesh.get("name").and_then(Value::as_str).unwrap_or("");
        if mname.ends_with("_LOD1") || mname.ends_with("_lod1") {
            continue;
        }
        let wm = world_mats.get(ni).copied().unwrap_or(id_mat);
        let Some(prims) = mesh.get("primitives").and_then(Value::as_array) else {
            continue;
        };
        for prim in prims {
            if prim.get("mode").and_then(Value::as_u64).unwrap_or(4) != 4 {
                continue;
            }
            let Some(attrs) = prim.get("attributes").and_then(Value::as_object) else {
                continue;
            };
            let Some(pos_acc) = attrs.get("POSITION").and_then(Value::as_u64) else {
                continue;
            };
            let (_, pos_flat) = read_f32_accessor(&json, bin, pos_acc as usize)?;
            let nv = pos_flat.len() / 3;
            let nrm_flat = attrs
                .get("NORMAL")
                .and_then(Value::as_u64)
                .and_then(|a| read_f32_accessor(&json, bin, a as usize).ok())
                .map(|(_, v)| v);
            let uv_flat = attrs
                .get("TEXCOORD_0")
                .and_then(Value::as_u64)
                .and_then(|a| read_f32_accessor(&json, bin, a as usize).ok())
                .map(|(_, v)| v);
            let col_acc = attrs
                .get("COLOR_0")
                .and_then(Value::as_u64)
                .and_then(|a| read_f32_accessor(&json, bin, a as usize).ok());
            let indices = match prim.get("indices").and_then(Value::as_u64) {
                Some(ia) => read_u32_indices(&json, bin, ia as usize)?,
                None => (0..nv as u32).collect(),
            };
            let mat_slot = prim
                .get("material")
                .and_then(Value::as_u64)
                .map(|m| m as usize + 1)
                .filter(|&m| m < mats.len())
                .unwrap_or(0);

            let tp = |vi: usize| -> [f32; 3] {
                let (px, py, pz) = (pos_flat[vi * 3], pos_flat[vi * 3 + 1], pos_flat[vi * 3 + 2]);
                [
                    wm[0] * px + wm[4] * py + wm[8] * pz + wm[12],
                    wm[1] * px + wm[5] * py + wm[9] * pz + wm[13],
                    wm[2] * px + wm[6] * py + wm[10] * pz + wm[14],
                ]
            };
            let tn = |vi: usize, fallback: [f32; 3]| -> [f32; 3] {
                if let Some(nf) = nrm_flat.as_ref().filter(|nf| vi * 3 + 2 < nf.len()) {
                    let (nx, ny, nz) = (nf[vi * 3], nf[vi * 3 + 1], nf[vi * 3 + 2]);
                    return normalize3([
                        wm[0] * nx + wm[4] * ny + wm[8] * nz,
                        wm[1] * nx + wm[5] * ny + wm[9] * nz,
                        wm[2] * nx + wm[6] * ny + wm[10] * nz,
                    ]);
                }
                fallback
            };
            let tuv = |vi: usize| -> [f32; 2] {
                uv_flat
                    .as_ref()
                    .filter(|u| vi * 2 + 1 < u.len())
                    .map(|u| [u[vi * 2], u[vi * 2 + 1]])
                    .unwrap_or([0.0, 0.0])
            };
            let tvc = |vi: usize| -> [f32; 4] {
                if let Some((nc, ref cf)) =
                    col_acc.as_ref().filter(|(nc, cf)| vi * nc + 2 < cf.len())
                {
                    return [
                        cf[vi * nc],
                        cf[vi * nc + 1],
                        cf[vi * nc + 2],
                        if *nc >= 4 { cf[vi * nc + 3] } else { 1.0 },
                    ];
                }
                [1.0, 1.0, 1.0, 1.0]
            };

            for tri in indices.chunks_exact(3) {
                let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
                if i0 >= nv || i1 >= nv || i2 >= nv {
                    continue;
                }
                let p = [tp(i0), tp(i1), tp(i2)];
                let e1 = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
                let e2 = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
                let fnrm = normalize3([
                    e1[1] * e2[2] - e1[2] * e2[1],
                    e1[2] * e2[0] - e1[0] * e2[2],
                    e1[0] * e2[1] - e1[1] * e2[0],
                ]);
                tris.push(TexturedTri {
                    p,
                    n: [tn(i0, fnrm), tn(i1, fnrm), tn(i2, fnrm)],
                    uv: [tuv(i0), tuv(i1), tuv(i2)],
                    vc: [tvc(i0), tvc(i1), tvc(i2)],
                    mat_idx: mat_slot,
                });
            }
        }
    }
    Ok((images, mats, tris))
}
