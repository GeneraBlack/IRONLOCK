use super::{
    DecodedPrimitive, CHUNK_BIN, CHUNK_JSON, COMP_BYTE, COMP_FLOAT, COMP_SHORT, COMP_UBYTE,
    COMP_UINT, COMP_USHORT, GLTF_MAGIC, GLTF_VERSION,
};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

/// Parses a binary glTF 2.0 container into `(json_root, bin_slice)`.
pub fn parse_glb_chunks(glb_bytes: &[u8]) -> Result<(Value, &[u8])> {
    if glb_bytes.len() < 20 {
        bail!("GLB buffer too short ({} bytes < 20)", glb_bytes.len());
    }
    let magic = u32::from_le_bytes(glb_bytes[0..4].try_into().unwrap());
    if magic != GLTF_MAGIC {
        bail!("Invalid GLB magic 0x{magic:08X} (expected 0x{GLTF_MAGIC:08X})");
    }
    let version = u32::from_le_bytes(glb_bytes[4..8].try_into().unwrap());
    if version != GLTF_VERSION {
        bail!("Unsupported GLB version {version} (expected 2)");
    }
    let total_len = u32::from_le_bytes(glb_bytes[8..12].try_into().unwrap()) as usize;
    if total_len > glb_bytes.len() || total_len < 20 {
        bail!(
            "Invalid GLB header length {} for file of {} bytes",
            total_len,
            glb_bytes.len()
        );
    }

    let json_len = u32::from_le_bytes(glb_bytes[12..16].try_into().unwrap()) as usize;
    let json_type = u32::from_le_bytes(glb_bytes[16..20].try_into().unwrap());
    if json_type != CHUNK_JSON {
        bail!("First GLB chunk is 0x{json_type:08X}, expected JSON (0x{CHUNK_JSON:08X})");
    }
    if json_len == 0 || 20 + json_len > total_len {
        bail!("Invalid GLB JSON chunk length {json_len}");
    }
    let json_val: Value = serde_json::from_slice(&glb_bytes[20..20 + json_len])
        .context("Failed to parse GLB JSON chunk")?;

    let mut cursor = 20 + json_len;
    let mut bin_slice: &[u8] = &[];
    if cursor + 8 <= total_len {
        let bin_len =
            u32::from_le_bytes(glb_bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
        let bin_type = u32::from_le_bytes(glb_bytes[cursor + 4..cursor + 8].try_into().unwrap());
        cursor += 8;
        if bin_type == CHUNK_BIN {
            if cursor + bin_len > total_len {
                bail!("GLB BIN chunk exceeds total file length");
            }
            bin_slice = &glb_bytes[cursor..cursor + bin_len];
        }
    }

    Ok((json_val, bin_slice))
}

/// Encodes `(json_root, bin_chunk)` into a 4-byte-aligned spec-compliant binary glTF 2.0 buffer.
pub fn encode_glb_chunks(mut json_root: Value, mut bin_chunk: Vec<u8>) -> Result<Vec<u8>> {
    while !bin_chunk.len().is_multiple_of(4) {
        bin_chunk.push(0);
    }
    if bin_chunk.is_empty() {
        json_root["buffers"] = json!([]);
    } else {
        json_root["buffers"] = json!([{ "byteLength": bin_chunk.len() }]);
    }

    let mut json_bytes = serde_json::to_vec(&json_root)?;
    while !json_bytes.len().is_multiple_of(4) {
        json_bytes.push(b' ');
    }

    let has_bin = !bin_chunk.is_empty();
    let total_len = 12 + 8 + json_bytes.len() + if has_bin { 8 + bin_chunk.len() } else { 0 };

    let mut out = Vec::with_capacity(total_len);
    out.extend_from_slice(&GLTF_MAGIC.to_le_bytes());
    out.extend_from_slice(&GLTF_VERSION.to_le_bytes());
    out.extend_from_slice(&(total_len as u32).to_le_bytes());

    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&CHUNK_JSON.to_le_bytes());
    out.extend_from_slice(&json_bytes);

    if has_bin {
        out.extend_from_slice(&(bin_chunk.len() as u32).to_le_bytes());
        out.extend_from_slice(&CHUNK_BIN.to_le_bytes());
        out.extend_from_slice(&bin_chunk);
    }

    Ok(out)
}

fn component_byte_size(comp_type: u32) -> Result<usize> {
    match comp_type {
        COMP_BYTE | COMP_UBYTE => Ok(1),
        COMP_SHORT | COMP_USHORT => Ok(2),
        COMP_UINT | COMP_FLOAT => Ok(4),
        other => bail!("Unsupported glTF accessor componentType {other}"),
    }
}

fn type_num_components(type_str: &str) -> Result<usize> {
    match type_str {
        "SCALAR" => Ok(1),
        "VEC2" => Ok(2),
        "VEC3" => Ok(3),
        "VEC4" | "MAT2" => Ok(4),
        "MAT3" => Ok(9),
        "MAT4" => Ok(16),
        other => bail!("Unsupported glTF accessor type '{other}'"),
    }
}

struct AccessorSlice<'a> {
    data: &'a [u8],
    stride: usize,
    count: usize,
    comp_type: u32,
    num_comp: usize,
}

fn resolve_accessor_slice<'a>(
    json: &Value,
    bin: &'a [u8],
    acc_idx: usize,
) -> Result<AccessorSlice<'a>> {
    let acc = json
        .get("accessors")
        .and_then(Value::as_array)
        .and_then(|a| a.get(acc_idx))
        .with_context(|| format!("Missing accessor index {acc_idx}"))?;
    let bv_idx =
        acc.get("bufferView")
            .and_then(Value::as_u64)
            .with_context(|| format!("Accessor {acc_idx} has no bufferView"))? as usize;
    let bv = json
        .get("bufferViews")
        .and_then(Value::as_array)
        .and_then(|b| b.get(bv_idx))
        .with_context(|| format!("Missing bufferView index {bv_idx}"))?;

    let comp_type = acc
        .get("componentType")
        .and_then(Value::as_u64)
        .context("Accessor missing componentType")? as u32;
    let type_str = acc
        .get("type")
        .and_then(Value::as_str)
        .context("Accessor missing type")?;
    let count = acc.get("count").and_then(Value::as_u64).unwrap_or(0) as usize;

    let comp_bytes = component_byte_size(comp_type)?;
    let num_comp = type_num_components(type_str)?;
    let elem_bytes = comp_bytes * num_comp;

    let bv_offset = bv.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let bv_len = bv.get("byteLength").and_then(Value::as_u64).unwrap_or(0) as usize;
    let acc_offset = acc.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let stride = bv
        .get("byteStride")
        .and_then(Value::as_u64)
        .map(|s| s as usize)
        .unwrap_or(elem_bytes);

    if bv_offset + bv_len > bin.len() {
        bail!(
            "BufferView {bv_idx} (offset={bv_offset}, len={bv_len}) exceeds BIN chunk ({} bytes)",
            bin.len()
        );
    }
    let start = bv_offset + acc_offset;
    if count > 0 {
        let needed_end = start + (count - 1) * stride + elem_bytes;
        if needed_end > bin.len() {
            bail!(
                "Accessor {acc_idx} range {start}..{needed_end} exceeds BIN chunk ({} bytes)",
                bin.len()
            );
        }
    }

    Ok(AccessorSlice {
        data: &bin[start.min(bin.len())..],
        stride,
        count,
        comp_type,
        num_comp,
    })
}

fn read_comp_as_f32(bytes: &[u8], comp_type: u32) -> f32 {
    match comp_type {
        COMP_FLOAT => f32::from_le_bytes(bytes[0..4].try_into().unwrap()),
        COMP_UBYTE => bytes[0] as f32 / 255.0,
        COMP_USHORT => u16::from_le_bytes(bytes[0..2].try_into().unwrap()) as f32 / 65535.0,
        COMP_BYTE => (bytes[0] as i8 as f32 / 127.0).max(-1.0),
        COMP_SHORT => {
            (i16::from_le_bytes(bytes[0..2].try_into().unwrap()) as f32 / 32767.0).max(-1.0)
        }
        COMP_UINT => u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as f32,
        _ => 0.0,
    }
}

pub(super) fn read_vec3_f32(json: &Value, bin: &[u8], acc_idx: usize) -> Result<Vec<[f32; 3]>> {
    let s = resolve_accessor_slice(json, bin, acc_idx)?;
    if s.num_comp < 3 {
        bail!("Expected VEC3 accessor at {acc_idx}");
    }
    let cs = component_byte_size(s.comp_type)?;
    let mut out = Vec::with_capacity(s.count);
    for i in 0..s.count {
        let base = i * s.stride;
        out.push([
            read_comp_as_f32(&s.data[base..base + cs], s.comp_type),
            read_comp_as_f32(&s.data[base + cs..base + 2 * cs], s.comp_type),
            read_comp_as_f32(&s.data[base + 2 * cs..base + 3 * cs], s.comp_type),
        ]);
    }
    Ok(out)
}

fn read_colors_rgba(json: &Value, bin: &[u8], acc_idx: usize) -> Result<Vec<[f32; 4]>> {
    let s = resolve_accessor_slice(json, bin, acc_idx)?;
    if s.num_comp < 3 {
        bail!("Expected VEC3 or VEC4 COLOR_0 accessor at {acc_idx}");
    }
    let cs = component_byte_size(s.comp_type)?;
    let mut out = Vec::with_capacity(s.count);
    for i in 0..s.count {
        let base = i * s.stride;
        let r = read_comp_as_f32(&s.data[base..base + cs], s.comp_type).clamp(0.0, 1.0);
        let g = read_comp_as_f32(&s.data[base + cs..base + 2 * cs], s.comp_type).clamp(0.0, 1.0);
        let b =
            read_comp_as_f32(&s.data[base + 2 * cs..base + 3 * cs], s.comp_type).clamp(0.0, 1.0);
        let a = if s.num_comp >= 4 {
            read_comp_as_f32(&s.data[base + 3 * cs..base + 4 * cs], s.comp_type).clamp(0.0, 1.0)
        } else {
            1.0
        };
        out.push([r, g, b, a]);
    }
    Ok(out)
}

fn read_texcoords(json: &Value, bin: &[u8], acc_idx: usize) -> Result<Vec<[f32; 2]>> {
    let s = resolve_accessor_slice(json, bin, acc_idx)?;
    if s.num_comp < 2 {
        bail!("Expected VEC2 TEXCOORD_0 accessor at {acc_idx}");
    }
    let cs = component_byte_size(s.comp_type)?;
    let mut out = Vec::with_capacity(s.count);
    for i in 0..s.count {
        let base = i * s.stride;
        out.push([
            read_comp_as_f32(&s.data[base..base + cs], s.comp_type),
            read_comp_as_f32(&s.data[base + cs..base + 2 * cs], s.comp_type),
        ]);
    }
    Ok(out)
}

fn read_indices_u32(json: &Value, bin: &[u8], acc_idx: usize) -> Result<Vec<u32>> {
    let s = resolve_accessor_slice(json, bin, acc_idx)?;
    let mut out = Vec::with_capacity(s.count);
    for i in 0..s.count {
        let base = i * s.stride;
        let idx = match s.comp_type {
            COMP_UBYTE => s.data[base] as u32,
            COMP_USHORT => u16::from_le_bytes(s.data[base..base + 2].try_into().unwrap()) as u32,
            COMP_UINT => u32::from_le_bytes(s.data[base..base + 4].try_into().unwrap()),
            other => bail!("Unsupported index componentType {other}"),
        };
        out.push(idx);
    }
    Ok(out)
}

pub(super) fn decode_primitive(json: &Value, bin: &[u8], prim: &Value) -> Result<DecodedPrimitive> {
    let attrs = prim
        .get("attributes")
        .and_then(Value::as_object)
        .context("Primitive missing attributes object")?;
    let pos_acc = attrs
        .get("POSITION")
        .and_then(Value::as_u64)
        .context("Primitive missing POSITION accessor")? as usize;
    let positions = read_vec3_f32(json, bin, pos_acc)?;
    let n_verts = positions.len();

    let colors = match attrs.get("COLOR_0").and_then(Value::as_u64) {
        Some(ca) => {
            let mut c = read_colors_rgba(json, bin, ca as usize)?;
            c.resize(n_verts, [1.0, 1.0, 1.0, 1.0]);
            Some(c)
        }
        None => None,
    };

    let texcoords = match attrs.get("TEXCOORD_0").and_then(Value::as_u64) {
        Some(ta) => {
            let mut uv = read_texcoords(json, bin, ta as usize)?;
            uv.resize(n_verts, [0.0, 0.0]);
            Some(uv)
        }
        None => None,
    };

    let indices = match prim.get("indices").and_then(Value::as_u64) {
        Some(ia) => read_indices_u32(json, bin, ia as usize)?,
        None => (0..(n_verts as u32)).collect(),
    };

    let mut normals = match attrs.get("NORMAL").and_then(Value::as_u64) {
        Some(na) => {
            let mut nrm = read_vec3_f32(json, bin, na as usize)?;
            nrm.resize(n_verts, [0.0, 0.0, 0.0]);
            nrm
        }
        None => vec![[0.0, 0.0, 0.0]; n_verts],
    };

    // Repair any zero-length / missing normals from triangle geometry
    let mut needs_repair = false;
    for n in &normals {
        let l2 = n[0] * n[0] + n[1] * n[1] + n[2] * n[2];
        if !l2.is_finite() || l2 < 1e-6 {
            needs_repair = true;
            break;
        }
    }
    if needs_repair {
        let mut accum = vec![[0.0f32; 3]; n_verts];
        for tri in indices.chunks_exact(3) {
            let [i0, i1, i2] = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
            if i0 >= n_verts || i1 >= n_verts || i2 >= n_verts {
                continue;
            }
            let p0 = positions[i0];
            let p1 = positions[i1];
            let p2 = positions[i2];
            let ux = p1[0] - p0[0];
            let uy = p1[1] - p0[1];
            let uz = p1[2] - p0[2];
            let vx = p2[0] - p0[0];
            let vy = p2[1] - p0[1];
            let vz = p2[2] - p0[2];
            let nx = uy * vz - uz * vy;
            let ny = uz * vx - ux * vz;
            let nz = ux * vy - uy * vx;
            for &vi in &[i0, i1, i2] {
                accum[vi][0] += nx;
                accum[vi][1] += ny;
                accum[vi][2] += nz;
            }
        }
        for (vi, n) in normals.iter_mut().enumerate() {
            let l2 = n[0] * n[0] + n[1] * n[1] + n[2] * n[2];
            if !l2.is_finite() || l2 < 1e-6 {
                let a = accum[vi];
                let al = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
                *n = if al > 1e-6 {
                    [a[0] / al, a[1] / al, a[2] / al]
                } else {
                    [0.0, 1.0, 0.0]
                };
            }
        }
    }

    let material = prim
        .get("material")
        .and_then(Value::as_u64)
        .map(|m| m as usize);

    Ok(DecodedPrimitive {
        positions,
        normals,
        colors,
        texcoords,
        indices,
        material,
    })
}

pub(super) fn append_encoded_primitive(
    prim: &DecodedPrimitive,
    bin_chunk: &mut Vec<u8>,
    buffer_views: &mut Vec<Value>,
    accessors: &mut Vec<Value>,
) -> Value {
    while !bin_chunk.len().is_multiple_of(4) {
        bin_chunk.push(0);
    }

    // 1. POSITION (VEC3 f32 with exact min/max bounds)
    let pos_off = bin_chunk.len();
    let mut bmin = [f32::MAX; 3];
    let mut bmax = [f32::MIN; 3];
    for p in &prim.positions {
        for a in 0..3 {
            bmin[a] = bmin[a].min(p[a]);
            bmax[a] = bmax[a].max(p[a]);
            bin_chunk.extend_from_slice(&p[a].to_le_bytes());
        }
    }
    if prim.positions.is_empty() {
        bmin = [0.0; 3];
        bmax = [0.0; 3];
    }
    let pos_bv = buffer_views.len();
    buffer_views.push(json!({
        "buffer": 0,
        "byteOffset": pos_off,
        "byteLength": bin_chunk.len() - pos_off,
        "target": 34962
    }));
    let pos_acc = accessors.len();
    accessors.push(json!({
        "bufferView": pos_bv,
        "componentType": COMP_FLOAT,
        "count": prim.positions.len(),
        "type": "VEC3",
        "min": bmin,
        "max": bmax
    }));

    // 2. NORMAL (VEC3 f32)
    let nrm_off = bin_chunk.len();
    for n in &prim.normals {
        for c in n {
            bin_chunk.extend_from_slice(&c.to_le_bytes());
        }
    }
    let nrm_bv = buffer_views.len();
    buffer_views.push(json!({
        "buffer": 0,
        "byteOffset": nrm_off,
        "byteLength": bin_chunk.len() - nrm_off,
        "target": 34962
    }));
    let nrm_acc = accessors.len();
    accessors.push(json!({
        "bufferView": nrm_bv,
        "componentType": COMP_FLOAT,
        "count": prim.normals.len(),
        "type": "VEC3"
    }));

    let mut attr_obj = serde_json::Map::new();
    attr_obj.insert("POSITION".to_string(), json!(pos_acc));
    attr_obj.insert("NORMAL".to_string(), json!(nrm_acc));

    // 3. COLOR_0 (VEC4 f32)
    if let Some(cols) = &prim.colors {
        let col_off = bin_chunk.len();
        for c in cols {
            for ch in c {
                bin_chunk.extend_from_slice(&ch.clamp(0.0, 1.0).to_le_bytes());
            }
        }
        let col_bv = buffer_views.len();
        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": col_off,
            "byteLength": bin_chunk.len() - col_off,
            "target": 34962
        }));
        let col_acc = accessors.len();
        accessors.push(json!({
            "bufferView": col_bv,
            "componentType": COMP_FLOAT,
            "count": cols.len(),
            "type": "VEC4"
        }));
        attr_obj.insert("COLOR_0".to_string(), json!(col_acc));
    }

    // 4. TEXCOORD_0 (VEC2 f32)
    if let Some(uvs) = &prim.texcoords {
        let uv_off = bin_chunk.len();
        for uv in uvs {
            bin_chunk.extend_from_slice(&uv[0].to_le_bytes());
            bin_chunk.extend_from_slice(&uv[1].to_le_bytes());
        }
        let uv_bv = buffer_views.len();
        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": uv_off,
            "byteLength": bin_chunk.len() - uv_off,
            "target": 34962
        }));
        let uv_acc = accessors.len();
        accessors.push(json!({
            "bufferView": uv_bv,
            "componentType": COMP_FLOAT,
            "count": uvs.len(),
            "type": "VEC2"
        }));
        attr_obj.insert("TEXCOORD_0".to_string(), json!(uv_acc));
    }

    // 5. Indices (u16 when < 65535 verts for compact GPU memory, u32 otherwise)
    let idx_off = bin_chunk.len();
    let use_u16 = prim.positions.len() < 65_535;
    let idx_comp_type = if use_u16 {
        for &idx in &prim.indices {
            bin_chunk.extend_from_slice(&(idx as u16).to_le_bytes());
        }
        COMP_USHORT
    } else {
        for &idx in &prim.indices {
            bin_chunk.extend_from_slice(&idx.to_le_bytes());
        }
        COMP_UINT
    };
    let idx_byte_len = bin_chunk.len() - idx_off;
    while !bin_chunk.len().is_multiple_of(4) {
        bin_chunk.push(0);
    }
    let idx_bv = buffer_views.len();
    buffer_views.push(json!({
        "buffer": 0,
        "byteOffset": idx_off,
        "byteLength": idx_byte_len,
        "target": 34963
    }));
    let idx_acc = accessors.len();
    accessors.push(json!({
        "bufferView": idx_bv,
        "componentType": idx_comp_type,
        "count": prim.indices.len(),
        "type": "SCALAR"
    }));

    let mut prim_obj = serde_json::Map::new();
    prim_obj.insert("attributes".to_string(), Value::Object(attr_obj));
    prim_obj.insert("indices".to_string(), json!(idx_acc));
    if let Some(mat) = prim.material {
        prim_obj.insert("material".to_string(), json!(mat));
    }
    Value::Object(prim_obj)
}
