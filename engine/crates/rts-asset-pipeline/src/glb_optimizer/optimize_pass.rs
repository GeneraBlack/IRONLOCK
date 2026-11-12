use super::binary_io::{
    append_encoded_primitive, decode_primitive, encode_glb_chunks, parse_glb_chunks,
};
use super::hierarchy::{
    distribute_vertex_budgets, preserve_non_mesh_buffers, strip_existing_lod1_hierarchy,
};
use super::inspect::inspect_glb_bytes;
use super::qem::simplify_primitive_qem;
use super::{DecodedPrimitive, GlbOptimizationReport, GlbOptimizeOptions};
use anyhow::{bail, Context, Result};
use rts_sdk::loader::LoadedMod;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Optimizes a single binary `.glb` asset in memory:
/// - Idempotently skips if `_LOD1` child nodes already exist and `LOD0` vertices `<= max_lod0_verts` (unless `force`).
/// - Simplifies each mesh primitive to `LOD0` (`<= max_lod0_verts`) and embeds a `LOD1` mesh (`~lod1_ratio * LOD0`)
///   under a `<NodeName>_LOD1` identity-transform child node of every mesh-bearing node.
pub fn optimize_glb_bytes(
    glb_bytes: &[u8],
    max_lod0_verts: usize,
    lod1_ratio: f32,
    force: bool,
) -> Result<(Vec<u8>, GlbOptimizationReport)> {
    let max_lod0_verts = max_lod0_verts.max(24);
    let lod1_ratio = lod1_ratio.clamp(0.10, 0.50);

    let pre_info = inspect_glb_bytes(glb_bytes)?;
    if !force && pre_info.has_lod1_for_all_meshes && pre_info.lod0_vertices <= max_lod0_verts {
        return Ok((
            glb_bytes.to_vec(),
            GlbOptimizationReport {
                original_vertices: pre_info.lod0_vertices,
                lod0_vertices: pre_info.lod0_vertices,
                lod1_vertices: pre_info.lod1_vertices,
                bounds_min: pre_info.bounds_min,
                bounds_max: pre_info.bounds_max,
                detected_sockets: pre_info.detected_sockets,
                animation_clips: pre_info.animation_clips,
                skipped_already_optimized: true,
            },
        ));
    }

    let (mut json, old_bin) = parse_glb_chunks(glb_bytes)?;
    strip_existing_lod1_hierarchy(&mut json);

    let old_meshes = json
        .get("meshes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if old_meshes.is_empty() {
        return Ok((
            glb_bytes.to_vec(),
            GlbOptimizationReport {
                original_vertices: 0,
                lod0_vertices: 0,
                lod1_vertices: 0,
                bounds_min: pre_info.bounds_min,
                bounds_max: pre_info.bounds_max,
                detected_sockets: pre_info.detected_sockets,
                animation_clips: pre_info.animation_clips,
                skipped_already_optimized: true,
            },
        ));
    }

    // Count how many nodes instantiate each mesh so shared meshes respect scene-wide vertex budget
    let mut mesh_inst_count = vec![0usize; old_meshes.len()];
    if let Some(nodes) = json.get("nodes").and_then(Value::as_array) {
        for node in nodes {
            if let Some(mi) = node.get("mesh").and_then(Value::as_u64) {
                if (mi as usize) < mesh_inst_count.len() {
                    mesh_inst_count[mi as usize] += 1;
                }
            }
        }
    }
    for c in &mut mesh_inst_count {
        if *c == 0 {
            *c = 1;
        }
    }

    let mut decoded_meshes: Vec<(String, Vec<DecodedPrimitive>)> =
        Vec::with_capacity(old_meshes.len());
    let mut weighted_prim_counts: Vec<usize> = Vec::new();
    let mut original_vertices = 0usize;

    for (mi, m) in old_meshes.iter().enumerate() {
        let mname = m
            .get("name")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .unwrap_or_else(|| format!("mesh_{mi}"));
        let mut prim_list = Vec::new();
        if let Some(prims) = m.get("primitives").and_then(Value::as_array) {
            for p in prims {
                let mode = p.get("mode").and_then(Value::as_u64).unwrap_or(4);
                if mode != 4 {
                    continue;
                }
                let dec = decode_primitive(&json, old_bin, p)?;
                original_vertices += dec.positions.len() * mesh_inst_count[mi];
                weighted_prim_counts.push(dec.positions.len() * mesh_inst_count[mi]);
                prim_list.push(dec);
            }
        }
        decoded_meshes.push((mname, prim_list));
    }

    let weighted_lod0_budgets =
        distribute_vertex_budgets(&weighted_prim_counts, max_lod0_verts, 48);

    let mut new_bin = Vec::new();
    let mut new_bvs = Vec::new();
    let mut new_accs = Vec::new();
    preserve_non_mesh_buffers(
        &mut json,
        old_bin,
        &mut new_bin,
        &mut new_bvs,
        &mut new_accs,
    );

    let mut lod0_meshes_json = Vec::with_capacity(decoded_meshes.len());
    let mut lod1_meshes_json = Vec::with_capacity(decoded_meshes.len());
    let mut lod0_decoded: Vec<Vec<DecodedPrimitive>> = Vec::with_capacity(decoded_meshes.len());

    let mut flat_idx = 0usize;
    let mut lod0_total_verts = 0usize;

    for (mi, (mname, prims)) in decoded_meshes.iter().enumerate() {
        let inst = mesh_inst_count[mi].max(1);
        let mut lod0_prims_json = Vec::with_capacity(prims.len());
        let mut lod0_prims_dec = Vec::with_capacity(prims.len());

        for prim in prims {
            let prim_budget = (weighted_lod0_budgets[flat_idx] / inst).max(6);
            flat_idx += 1;
            let simp0 = simplify_primitive_qem(prim, prim_budget);
            lod0_total_verts += simp0.positions.len() * inst;
            lod0_prims_json.push(append_encoded_primitive(
                &simp0,
                &mut new_bin,
                &mut new_bvs,
                &mut new_accs,
            ));
            lod0_prims_dec.push(simp0);
        }

        lod0_meshes_json.push(json!({
            "name": mname,
            "primitives": lod0_prims_json
        }));
        lod0_decoded.push(lod0_prims_dec);
    }

    // Compute total LOD1 target (~lod1_ratio of LOD0, clamped cleanly to 400..2500 when LOD0 >= 400)
    let raw_lod1_total = ((lod0_total_verts as f32) * lod1_ratio).round() as usize;
    let lod1_total_budget = if lod0_total_verts >= 1_600 {
        raw_lod1_total.clamp(400, 2_500)
    } else if lod0_total_verts >= 450 {
        raw_lod1_total.clamp(360.min(lod0_total_verts), 2_500)
    } else {
        ((lod0_total_verts * 3) / 5).max(18.min(lod0_total_verts))
    };

    let weighted_lod0_actual: Vec<usize> = lod0_decoded
        .iter()
        .enumerate()
        .flat_map(|(mi, ps)| {
            let inst = mesh_inst_count[mi].max(1);
            ps.iter().map(move |p| p.positions.len() * inst)
        })
        .collect();
    let weighted_lod1_budgets =
        distribute_vertex_budgets(&weighted_lod0_actual, lod1_total_budget, 24);

    let mut flat_lod1_idx = 0usize;
    let mut lod1_total_verts = 0usize;

    for (mi, (mname, orig_prims)) in decoded_meshes.iter().enumerate() {
        let inst = mesh_inst_count[mi].max(1);
        let mut lod1_prims_json = Vec::with_capacity(orig_prims.len());
        for orig_p in orig_prims {
            let p_budget = (weighted_lod1_budgets[flat_lod1_idx] / inst).max(6);
            flat_lod1_idx += 1;
            let simp1 = simplify_primitive_qem(orig_p, p_budget);
            lod1_total_verts += simp1.positions.len() * inst;
            lod1_prims_json.push(append_encoded_primitive(
                &simp1,
                &mut new_bin,
                &mut new_bvs,
                &mut new_accs,
            ));
        }
        lod1_meshes_json.push(json!({
            "name": format!("{mname}_LOD1"),
            "primitives": lod1_prims_json
        }));
    }

    let base_mesh_count = lod0_meshes_json.len();
    let mut combined_meshes = lod0_meshes_json;
    combined_meshes.extend(lod1_meshes_json);
    json["meshes"] = Value::Array(combined_meshes);
    json["bufferViews"] = Value::Array(new_bvs);
    json["accessors"] = Value::Array(new_accs);

    // Attach `<NodeName>_LOD1` child nodes (identity local transform) under every mesh-bearing node
    if let Some(nodes) = json.get("nodes").and_then(Value::as_array).cloned() {
        let orig_len = nodes.len();
        let mut updated_nodes = nodes;
        for ni in 0..orig_len {
            let Some(mesh_idx) = updated_nodes[ni]
                .get("mesh")
                .and_then(Value::as_u64)
                .map(|v| v as usize)
            else {
                continue;
            };
            if mesh_idx >= base_mesh_count {
                continue;
            }
            let base_name = updated_nodes[ni]
                .get("name")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .unwrap_or_else(|| format!("Node_{ni}"));
            let lod1_node_idx = updated_nodes.len();
            let lod1_mesh_idx = base_mesh_count + mesh_idx;
            updated_nodes.push(json!({
                "name": format!("{base_name}_LOD1"),
                "mesh": lod1_mesh_idx
            }));

            let children_val = updated_nodes[ni]
                .as_object_mut()
                .expect("glTF node must be JSON object")
                .entry("children")
                .or_insert_with(|| json!([]));
            if let Some(arr) = children_val.as_array_mut() {
                arr.push(json!(lod1_node_idx));
            }
        }
        json["nodes"] = Value::Array(updated_nodes);
    }

    if let Some(asset) = json.get_mut("asset").and_then(Value::as_object_mut) {
        asset.insert("version".to_string(), json!("2.0"));
        asset.insert(
            "generator".to_string(),
            json!("rts-mod-tools glb_optimizer v1 (QEM LOD0+LOD1)"),
        );
    } else {
        json["asset"] = json!({
            "version": "2.0",
            "generator": "rts-mod-tools glb_optimizer v1 (QEM LOD0+LOD1)"
        });
    }

    let encoded = encode_glb_chunks(json, new_bin)?;
    let post_info = inspect_glb_bytes(&encoded)?;

    Ok((
        encoded,
        GlbOptimizationReport {
            original_vertices,
            lod0_vertices: lod0_total_verts,
            lod1_vertices: lod1_total_verts,
            bounds_min: post_info.bounds_min,
            bounds_max: post_info.bounds_max,
            detected_sockets: post_info.detected_sockets,
            animation_clips: post_info.animation_clips,
            skipped_already_optimized: false,
        },
    ))
}

pub(super) fn collect_glb_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            collect_glb_files_recursive(&p, out)?;
        } else if p
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("glb"))
        {
            out.push(p);
        }
    }
    Ok(())
}

fn is_landmark_or_structure_stem(stem: &str) -> bool {
    let s = stem.to_ascii_lowercase();
    s.contains("conyard")
        || s.contains("barracks")
        || s.contains("refinery")
        || s.contains("factory")
        || s.contains("reactor")
        || s.contains("plant")
        || s.contains("shipyard")
        || s.contains("battle_lab")
        || s.contains("depot")
        || s.contains("radar")
        || s.contains("silo")
        || s.contains("curtain")
        || s.contains("chronosphere")
        || s.contains("weather")
        || s.contains("dominator")
        || s.contains("mutator")
        || s.contains("cloning")
        || s.contains("grinder")
        || s.contains("submarine_pen")
        || s.contains("bunker")
        || s.contains("tower")
        || s.contains("cannon")
        || s.contains("pillbox")
        || s.contains("sentry")
        || s.contains("missile") && s.contains("patriot")
        || s.contains("chapel")
        || s.contains("prison")
        || s.contains("palace")
        || s.contains("cathedral")
        || s.contains("pyramid")
        || s.contains("white_house")
        || s.contains("big_ben")
        || s.contains("liberty")
        || s.contains("fortress") && s.contains("transylvania")
        || s.contains("einstein_lab")
        || s.contains("time_machine")
        || s.contains("psychic_amplifier")
        || s.contains("psychic_beacon")
        || s.contains("tech_")
        || s.contains("civilian_building")
}

/// Optimizes all `.glb` files under `<mod_path>/assets/models/**` in parallel across CPU cores,
/// assigning building vs. unit vertex budgets deterministically from the mod's catalogs.
pub fn optimize_mod_models(
    mod_path: &Path,
    opts: &GlbOptimizeOptions,
) -> Result<Vec<(PathBuf, GlbOptimizationReport)>> {
    let models_dir = mod_path.join("assets/models");
    if !models_dir.is_dir() {
        bail!("Models directory does not exist: {}", models_dir.display());
    }

    let mut building_models = BTreeSet::new();
    let mut unit_models = BTreeSet::new();
    if let Ok(loaded) = LoadedMod::load_from_dir(mod_path) {
        for b in loaded.buildings.values() {
            building_models.insert(b.visual.model.replace('\\', "/").to_ascii_lowercase());
        }
        for u in loaded.units.values() {
            unit_models.insert(u.visual.model.replace('\\', "/").to_ascii_lowercase());
        }
    }

    let mut glb_files = Vec::new();
    collect_glb_files_recursive(&models_dir, &mut glb_files)?;
    glb_files.sort();

    let jobs: Vec<(PathBuf, usize)> = glb_files
        .into_iter()
        .map(|path| {
            let rel = path
                .strip_prefix(mod_path)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/")
                .to_ascii_lowercase();
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let budget = if unit_models.contains(&rel) {
                opts.max_unit_verts
            } else if building_models.contains(&rel) || is_landmark_or_structure_stem(stem) {
                opts.max_building_verts
            } else {
                opts.max_unit_verts
            };
            (path, budget)
        })
        .collect();

    let worker_count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(1, 16);
    let next_idx = std::sync::atomic::AtomicUsize::new(0);
    let results: std::sync::Mutex<Vec<(usize, PathBuf, Result<GlbOptimizationReport>)>> =
        std::sync::Mutex::new(Vec::with_capacity(jobs.len()));

    std::thread::scope(|scope| {
        for _ in 0..worker_count.min(jobs.len().max(1)) {
            scope.spawn(|| loop {
                let i = next_idx.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if i >= jobs.len() {
                    break;
                }
                let (ref path, budget) = jobs[i];
                let res = (|| -> Result<GlbOptimizationReport> {
                    let raw = fs::read(path)
                        .with_context(|| format!("Failed to read {}", path.display()))?;
                    let (optimized, rep) =
                        optimize_glb_bytes(&raw, budget, opts.lod1_ratio, opts.force)?;
                    if !rep.skipped_already_optimized {
                        let mut last_err = None;
                        for attempt in 0..15 {
                            match fs::write(path, &optimized) {
                                Ok(()) => {
                                    last_err = None;
                                    break;
                                }
                                Err(e) => {
                                    last_err = Some(e);
                                    std::thread::sleep(std::time::Duration::from_millis(
                                        60 + attempt * 25,
                                    ));
                                }
                            }
                        }
                        if let Some(e) = last_err {
                            return Err(e)
                                .with_context(|| format!("Failed to write {}", path.display()));
                        }
                    }
                    Ok(rep)
                })();
                results.lock().unwrap().push((i, path.clone(), res));
            });
        }
    });

    let mut collected = results.into_inner().unwrap();
    collected.sort_by_key(|(i, _, _)| *i);

    let mut final_reports = Vec::with_capacity(collected.len());
    for (_, path, res) in collected {
        final_reports.push((path, res?));
    }
    Ok(final_reports)
}
